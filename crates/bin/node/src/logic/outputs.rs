use std::collections::HashSet;

use cashu_signer::SignBlindedMessagesRequest;
use cashu_starknet::Unit;
use memory_db::InsertBlindSignaturesQueryBuilder;
use num_traits::CheckedAdd;
use nuts::{
    nut00::{BlindSignature, BlindedMessage},
    nut01::PublicKey,
    Amount,
};
use sqlx::PgConnection;

use crate::{
    app_state::SharedSignerClient,
    errors::{BlindMessageError, Error, QuoteError, SignerError, SwapError},
    keyset_cache::KeysetCache,
};

pub async fn check_outputs_allow_single_unit(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    outputs: &[BlindedMessage],
) -> Result<Amount, Error> {
    let mut blind_secrets = HashSet::with_capacity(outputs.len());
    let mut total_amount = Amount::ZERO;
    let mut unit = None;

    for blind_message in outputs {
        // Uniqueness
        if !blind_secrets.insert(blind_message.blinded_secret) {
            Err(SwapError::DuplicateOutput)?;
        }

        let keyset_info = keyset_cache
            .get_keyset_info(conn, blind_message.keyset_id)
            .await?;

        // We only sign with active keysets
        if !keyset_info.active() {
            return Err(Error::InactiveKeyset);
        }

        match (unit, keyset_info.unit()) {
            (None, u) => unit = Some(u),
            (Some(unit), u) if u != unit => return Err(QuoteError::MultipleUnits.into()),
            _ => {}
        }

        // Incement total amount
        total_amount = total_amount
            .checked_add(&blind_message.amount)
            .ok_or(Error::Overflow)?;
    }

    // Make sure those outputs were not already signed
    if memory_db::is_any_blind_message_already_used(conn, blind_secrets.into_iter()).await? {
        Err(BlindMessageError::AlreadySigned)?;
    }

    Ok(total_amount)
}

pub async fn check_outputs_allow_multiple_units(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    outputs: &[BlindedMessage],
) -> Result<Vec<(Unit, Amount)>, Error> {
    let mut blind_secrets = HashSet::with_capacity(outputs.len());
    let mut total_amounts: Vec<(Unit, Amount)> = Vec::new();

    for blind_message in outputs {
        // Uniqueness
        if !blind_secrets.insert(blind_message.blinded_secret) {
            Err(SwapError::DuplicateOutput)?;
        }

        let keyset_info = keyset_cache
            .get_keyset_info(conn, blind_message.keyset_id)
            .await?;

        // We only sign with active keysets
        if !keyset_info.active() {
            return Err(Error::InactiveKeyset);
        }

        // Incement total amount
        let keyset_unit = keyset_info.unit();
        match total_amounts.iter_mut().find(|(u, _)| *u == keyset_unit) {
            Some((_, a)) => {
                *a = a
                    .checked_add(&blind_message.amount)
                    .ok_or(Error::Overflow)?
            }
            None => total_amounts.push((keyset_unit, blind_message.amount)),
        }
    }

    // Make sure those outputs were not already signed
    if memory_db::is_any_blind_message_already_used(conn, blind_secrets.into_iter()).await? {
        Err(BlindMessageError::AlreadySigned)?;
    }

    Ok(total_amounts)
}

pub async fn process_outputs<'a>(
    signer: SharedSignerClient,
    outputs: &[BlindedMessage],
) -> Result<(Vec<BlindSignature>, InsertBlindSignaturesQueryBuilder<'a>), Error> {
    let mut query_builder = InsertBlindSignaturesQueryBuilder::new();

    let blind_signatures = {
        let mut signer_write_lock = signer.write().await;
        signer_write_lock
            .sign_blinded_messages(SignBlindedMessagesRequest {
                messages: outputs
                    .iter()
                    .map(|bm| cashu_signer::BlindedMessage {
                        amount: bm.amount.into(),
                        keyset_id: bm.keyset_id.to_bytes().to_vec(),
                        blinded_secret: bm.blinded_secret.to_bytes().to_vec(),
                    })
                    .collect(),
            })
            .await?
            .into_inner()
            .signatures
    };

    let blind_signatures = outputs
        .iter()
        .zip(blind_signatures)
        .map(|(bm, bs)| -> Result<_, SignerError> {
            let blind_signature = BlindSignature {
                amount: bm.amount,
                keyset_id: bm.keyset_id,
                c: PublicKey::from_slice(&bs)?,
            };

            query_builder.add_row(bm.blinded_secret, &blind_signature);

            Ok(blind_signature)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((blind_signatures, query_builder))
}

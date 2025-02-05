use std::collections::HashSet;

use keys_manager::KeysManager;
use memory_db::InsertBlindSignaturesQueryBuilder;
use num_traits::CheckedAdd;
use nuts::{
    dhke::sign_message,
    nut00::{BlindSignature, BlindedMessage},
    Amount,
};
use sqlx::PgConnection;

use crate::{
    errors::{Error, QuoteError, SwapError},
    keyset_cache::KeysetCache,
    Unit,
};

pub async fn process_outputs_allow_single_unit(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    outputs: &[BlindedMessage],
) -> Result<Amount, Error> {
    let mut blind_secrets = HashSet::with_capacity(outputs.len());
    let mut total_amount = Amount::ZERO;
    let mut unit = None;

    for blind_message in outputs {
        // Uniqueness
        if !blind_secrets.insert(blind_message.blind_secret) {
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
        Err(SwapError::BlindMessageAlreadySigned)?;
    }

    Ok(total_amount)
}

pub async fn process_outputs_allow_multiple_units(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    outputs: &[BlindedMessage],
) -> Result<Vec<(Unit, Amount)>, Error> {
    let mut blind_secrets = HashSet::with_capacity(outputs.len());
    let mut total_amounts: Vec<(Unit, Amount)> = Vec::new();

    for blind_message in outputs {
        // Uniqueness
        if !blind_secrets.insert(blind_message.blind_secret) {
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
        Err(SwapError::BlindMessageAlreadySigned)?;
    }

    Ok(total_amounts)
}

pub async fn process_outputs<'a>(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    keys_manager: &KeysManager,
    outputs: &[BlindedMessage],
) -> Result<(Vec<BlindSignature>, InsertBlindSignaturesQueryBuilder<'a>), Error> {
    let mut blind_signatures = Vec::with_capacity(outputs.len());
    let mut query_builder = InsertBlindSignaturesQueryBuilder::new();

    for blind_message in outputs {
        let key_pair = keyset_cache
            .get_key(
                conn,
                keys_manager,
                blind_message.keyset_id,
                &blind_message.amount,
            )
            .await?;

        let c = sign_message(&key_pair.secret_key, &blind_message.blind_secret)?;
        let blind_signature = BlindSignature {
            amount: blind_message.amount,
            keyset_id: blind_message.keyset_id,
            c,
        };

        query_builder.add_row(blind_message.blind_secret, &blind_signature);
        blind_signatures.push(blind_signature);
    }

    Ok((blind_signatures, query_builder))
}

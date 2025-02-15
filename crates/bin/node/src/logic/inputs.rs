use cashu_starknet::Unit;
use futures::TryFutureExt;
use num_traits::CheckedAdd;
use std::collections::HashSet;

use cashu_signer::VerifyProofsRequest;
use memory_db::InsertSpentProofsQueryBuilder;
use nuts::{nut00::Proof, nut01::PublicKey, Amount};
use sqlx::PgConnection;

use crate::{
    app_state::SharedSignerClient,
    errors::{Error, MeltError, ProofError, SwapError},
    keyset_cache::KeysetCache,
};

pub async fn process_melt_inputs<'a>(
    conn: &mut PgConnection,
    signer: SharedSignerClient,
    keyset_cache: &mut KeysetCache,
    inputs: &'a [Proof],
    expected_unit: Unit,
) -> Result<(Amount, InsertSpentProofsQueryBuilder<'a>), Error> {
    let mut secrets = HashSet::new();
    let mut query_builder = InsertSpentProofsQueryBuilder::new();
    let mut total_amount = Amount::ZERO;

    let mut verify_proofs_request = Vec::with_capacity(inputs.len());

    for proof in inputs {
        // Uniqueness
        if !secrets.insert(proof.y().map_err(|_| Error::HashOnCurve)?) {
            Err(SwapError::DuplicateInput)?;
        }

        let keyset_info = keyset_cache.get_keyset_info(conn, proof.keyset_id).await?;

        // Compute and increment fee
        let unit = keyset_info.unit();
        if expected_unit != unit {
            return Err(MeltError::InvalidInputsUnit(expected_unit, unit).into());
        }
        // Incement total amount
        total_amount = total_amount
            .checked_add(&proof.amount)
            .ok_or(Error::Overflow)?;

        // Append to insert query
        query_builder.add_row(proof)?;

        // Prepare payload for verification
        verify_proofs_request.push(cashu_signer::Proof {
            amount: proof.amount.into(),
            keyset_id: proof.keyset_id.to_bytes().to_vec(),
            secret: proof.secret.to_string(),
            unblind_signature: proof.c.to_bytes().to_vec(),
        });
    }

    run_verification_queries(conn, secrets, signer, verify_proofs_request).await?;

    Ok((total_amount, query_builder))
}

pub async fn process_swap_inputs<'a>(
    conn: &mut PgConnection,
    signer: SharedSignerClient,
    keyset_cache: &mut KeysetCache,
    inputs: &'a [Proof],
) -> Result<(Vec<(Unit, u16, Amount)>, InsertSpentProofsQueryBuilder<'a>), Error> {
    // Input process
    let mut secrets = HashSet::new();
    let mut fees_and_amounts: Vec<(Unit, u16, Amount)> = Vec::new();
    let mut query_builder = InsertSpentProofsQueryBuilder::new();

    let mut verify_proofs_request = Vec::with_capacity(inputs.len());

    for proof in inputs {
        // Uniqueness
        if !secrets.insert(proof.y().map_err(|_| Error::HashOnCurve)?) {
            Err(SwapError::DuplicateInput)?;
        }

        let keyset_info = keyset_cache.get_keyset_info(conn, proof.keyset_id).await?;

        // Compute and increment fee
        let fee_for_this_proof = (keyset_info.input_fee_ppk() + 999) / 1000;
        let keyset_unit = keyset_info.unit();
        match fees_and_amounts
            .iter_mut()
            .find(|(u, _, _)| *u == keyset_unit)
        {
            Some((_, f, a)) => {
                *f = f.checked_add(fee_for_this_proof).ok_or(Error::Overflow)?;
                *a = a.checked_add(&proof.amount).ok_or(Error::Overflow)?;
            }
            None => fees_and_amounts.push((keyset_unit, fee_for_this_proof, proof.amount)),
        }

        // Append to insert query
        query_builder.add_row(proof)?;

        // Prepare payload for verification
        verify_proofs_request.push(cashu_signer::Proof {
            keyset_id: proof.keyset_id.to_bytes().to_vec(),
            amount: proof.amount.into(),
            secret: proof.secret.to_string(),
            unblind_signature: proof.c.to_bytes().to_vec(),
        });
    }

    run_verification_queries(conn, secrets, signer, verify_proofs_request).await?;

    Ok((fees_and_amounts, query_builder))
}

async fn run_verification_queries(
    conn: &mut PgConnection,
    secrets: HashSet<PublicKey>,
    signer: SharedSignerClient,
    verify_proofs_request: Vec<cashu_signer::Proof>,
) -> Result<(), Error> {
    let query_signer_future = async {
        let mut lock = signer.write().await;
        lock.verify_proofs(VerifyProofsRequest {
            proofs: verify_proofs_request,
        })
        .await
    };

    // Parrallelize the two calls
    let res = tokio::try_join!(
        // Make sure those proof are valid
        query_signer_future
            .map_err(Error::from)
            .map_ok(|r| r.get_ref().is_valid),
        // Make sure those inputs were not already used
        memory_db::is_any_proof_already_used(conn, secrets.into_iter()).map_err(Error::from),
    );

    match res {
        Ok((false, _)) => Err(ProofError::Invalid.into()),
        Ok((_, true)) => Err(ProofError::Used.into()),
        Err(e) => Err(e),
        Ok((true, false)) => Ok(()),
    }
}

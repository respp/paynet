use num_traits::CheckedAdd;
use std::collections::HashSet;

use keys_manager::KeysManager;
use memory_db::InsertSpentProofsQueryBuilder;
use nuts::{dhke::verify_message, nut00::Proof, Amount};
use sqlx::PgConnection;

use crate::{
    errors::{Error, MeltError, SwapError},
    keyset_cache::KeysetCache,
    Unit,
};

pub async fn process_melt_inputs<'a>(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    keys_manager: &KeysManager,
    inputs: &'a [Proof],
    expected_unit: Unit,
) -> Result<(Amount, InsertSpentProofsQueryBuilder<'a>), Error> {
    let mut secrets = HashSet::new();
    let mut query_builder = InsertSpentProofsQueryBuilder::new();
    let mut total_amount = Amount::ZERO;

    for proof in inputs {
        // Uniqueness
        if !secrets.insert(proof.y().map_err(|_| Error::HashOnCurve)?) {
            Err(SwapError::DuplicateInput)?;
        }

        let keyset_info = keyset_cache.get_keyset_info(conn, proof.keyset_id).await?;

        // Make sure the proof belong to an existing keyset and is valid
        let keypair = keyset_cache
            .get_key(conn, keys_manager, proof.keyset_id, &proof.amount)
            .await?;
        verify_message(&keypair.secret_key, proof.c, proof.secret.as_bytes())?;

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
    }

    // Make sure those inputs were not already used
    if memory_db::is_any_proof_already_used(conn, secrets.into_iter()).await? {
        Err(SwapError::BlindMessageAlreadySigned)?;
    }

    Ok((total_amount, query_builder))
}

pub async fn process_swap_inputs<'a>(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    keys_manager: &KeysManager,
    inputs: &'a [Proof],
) -> Result<(Vec<(Unit, u16, Amount)>, InsertSpentProofsQueryBuilder<'a>), Error> {
    // Input process
    let mut secrets = HashSet::new();
    let mut fees_and_amounts: Vec<(Unit, u16, Amount)> = Vec::new();
    let mut query_builder = InsertSpentProofsQueryBuilder::new();

    for proof in inputs {
        // Uniqueness
        if !secrets.insert(proof.y().map_err(|_| Error::HashOnCurve)?) {
            Err(SwapError::DuplicateInput)?;
        }

        let keyset_info = keyset_cache.get_keyset_info(conn, proof.keyset_id).await?;

        // Make sure the proof belong to an existing keyset and is valid
        let keypair = keyset_cache
            .get_key(conn, keys_manager, proof.keyset_id, &proof.amount)
            .await?;
        verify_message(&keypair.secret_key, proof.c, proof.secret.as_bytes())?;

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
    }

    // Make sure those inputs were not already used
    if memory_db::is_any_proof_already_used(conn, secrets.into_iter()).await? {
        Err(SwapError::BlindMessageAlreadySigned)?;
    }

    Ok((fees_and_amounts, query_builder))
}

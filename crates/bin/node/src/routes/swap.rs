use std::collections::HashSet;

use axum::{extract::State, Json};
use keys_manager::KeysManager;
use memory_db::InsertSpentProofsQueryBuilder;
use num_traits::CheckedAdd;
use nuts::{
    dhke::verify_message,
    nut00::Proof,
    nut03::{SwapRequest, SwapResponse},
    Amount,
};
use sqlx::{PgConnection, PgPool};

use crate::{
    errors::{Error, SwapError},
    keyset_cache::KeysetCache,
    logic::{process_outputs, verify_outputs_allow_multiple_units},
    Unit,
};

async fn process_inputs<'a>(
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

pub async fn swap(
    State(pool): State<PgPool>,
    State(mut keyset_cache): State<KeysetCache>,
    State(keys_manager): State<KeysManager>,
    Json(swap_request): Json<SwapRequest>,
) -> Result<Json<SwapResponse>, Error> {
    let mut tx = pool.begin().await?;

    memory_db::set_transaction_isolation_level_to_serializable(&mut tx).await?;

    // First light (not io/computational intensive) verification
    // We:
    // - check they are not already used in db
    // - compute their respective total amount
    // This way we won't waste the rest of the work if those easy ones fail
    let outputs_amounts =
        verify_outputs_allow_multiple_units(&mut tx, &mut keyset_cache, &swap_request.outputs)
            .await?;

    // Second round of verification and process
    // This is a bit more computational intensive
    // and requires to access the cache (which can trigger db reads and async waiting)
    let (input_fees_and_amount, insert_spent_proofs_query_builder) = process_inputs(
        &mut tx,
        &mut keyset_cache,
        &keys_manager,
        &swap_request.inputs,
    )
    .await?;

    // Amount matching
    for (unit, output_amount) in outputs_amounts {
        let &(_, fee, input_amount) = input_fees_and_amount
            .iter()
            .find(|(u, _, _)| *u == unit)
            .ok_or(SwapError::UnbalancedUnits)?;

        if input_amount
            != output_amount
                .checked_add(&Amount::from(fee))
                .ok_or(Error::Overflow)?
        {
            Err(SwapError::TransactionUnbalanced(
                unit,
                input_amount,
                output_amount,
                fee,
            ))?;
        }
    }

    // Output process
    let (blind_signatures, insert_blind_signatures_query_builder) = process_outputs(
        &mut tx,
        &mut keyset_cache,
        &keys_manager,
        &swap_request.outputs,
    )
    .await?;

    insert_spent_proofs_query_builder.execute(&mut tx).await?;
    insert_blind_signatures_query_builder
        .execute(&mut tx)
        .await?;

    tx.commit().await?;

    Ok(Json(SwapResponse {
        signatures: blind_signatures,
    }))
}

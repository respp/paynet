use std::collections::HashSet;

use axum::{extract::State, Json};
use keys_manager::KeysManager;
use memory_db::{InsertBlindSignaturesQueryBuilder, InsertSpentProofsQueryBuilder};
use num_traits::CheckedAdd;
use nuts::{
    dhke::{sign_message, verify_message},
    nut00::{BlindMessage, BlindSignature, Proof},
    nut03::{PostSwapRequest, PostSwapResponse},
    Amount,
};
use sqlx::{PgConnection, PgPool};

use crate::{errors::Error, keyset_cache::KeysetCache};

async fn outputs_verification(
    conn: &mut PgConnection,
    outputs: &[BlindMessage],
) -> Result<Amount, Error> {
    let mut blind_secrets = HashSet::with_capacity(outputs.len());
    let mut total_amount = Amount::ZERO;

    for output in outputs {
        // Uniqueness
        if !blind_secrets.insert(output.blind_secret) {
            return Err(Error::DuplicateOutput);
        }

        // Incement total amount
        total_amount = total_amount
            .checked_add(&output.amount)
            .ok_or(Error::Overflow)?;
    }

    // Make sure those outputs were not already signed
    if memory_db::is_any_blind_message_already_used(conn, blind_secrets.into_iter()).await? {
        return Err(Error::BlindMessageAlreadySigned);
    }

    Ok(total_amount)
}

async fn inputs_verification(conn: &mut PgConnection, inputs: &[Proof]) -> Result<Amount, Error> {
    let mut secrets = HashSet::new();
    let mut total_amount = Amount::ZERO;

    for input in inputs {
        // Uniqueness
        if !secrets.insert(input.y().map_err(|_| Error::HashOnCurve)?) {
            return Err(Error::DuplicateOutput);
        }

        // Incement total amount
        total_amount = total_amount
            .checked_add(&input.amount)
            .ok_or(Error::Overflow)?;
    }

    // Make sure those inputs were not already used
    if memory_db::is_any_proof_already_used(conn, secrets.into_iter()).await? {
        return Err(Error::BlindMessageAlreadySigned);
    }

    Ok(total_amount)
}

async fn process_inputs<'a>(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    keys_manager: &KeysManager,
    inputs: &'a [Proof],
) -> Result<(u16, InsertSpentProofsQueryBuilder<'a>), Error> {
    // Input process
    let mut fee: u16 = 0;
    let mut query_builder = InsertSpentProofsQueryBuilder::new();
    let mut unit = None;

    for proof in inputs {
        let keyset_info = keyset_cache.get_keyset_info(conn, proof.keyset_id).await?;

        // TODO: Safely support different units
        match (unit, keyset_info.unit()) {
            (None, u) => unit = Some(u),
            (Some(u1), u2) if u1 != u2 => return Err(Error::MultipleUnits),
            _ => {}
        }

        // Make sure the proof belong to an existing keyset and is valid
        let keypair = keyset_cache
            .get_key(conn, keys_manager, proof.keyset_id, &proof.amount)
            .await?;
        verify_message(&keypair.secret_key, proof.c, proof.secret.as_bytes())?;

        // Compute and increment fee
        let fee_for_this_proof = (keyset_info.input_fee_ppk() + 999) / 1000;
        fee = fee.checked_add(fee_for_this_proof).ok_or(Error::Overflow)?;

        // Append to insert query
        query_builder.add_row(proof)?;
    }

    Ok((fee, query_builder))
}

async fn process_outputs<'a>(
    conn: &mut PgConnection,
    keyset_cache: &mut KeysetCache,
    keys_manager: &KeysManager,
    outputs: &[BlindMessage],
) -> Result<(Vec<BlindSignature>, InsertBlindSignaturesQueryBuilder<'a>), Error> {
    let mut blind_signatures = Vec::with_capacity(outputs.len());
    let mut query_builder = InsertBlindSignaturesQueryBuilder::new();
    let mut unit = None;

    for blind_message in outputs {
        let keyset_info = keyset_cache
            .get_keyset_info(conn, blind_message.keyset_id)
            .await?;

        // We only sign with active keysets
        if !keyset_info.active() {
            return Err(Error::InactiveKeyset);
        }

        // TODO: Safely support different units
        match (unit, keyset_info.unit()) {
            (None, u) => unit = Some(u),
            (Some(u1), u2) if u1 != u2 => return Err(Error::MultipleUnits),
            _ => {}
        }

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

// #[axum::debug_handler(state = KeysManager)]
pub async fn swap(
    State(pool): State<PgPool>,
    State(mut keyset_cache): State<KeysetCache>,
    State(keys_manager): State<KeysManager>,
    Json(swap_request): Json<PostSwapRequest>,
) -> Result<Json<PostSwapResponse>, Error> {
    let mut tx = pool.begin().await?;

    // Handle concurency at the database level
    // If one transaction alter a field that is used in another one
    // in a way that would result in a different statement output,
    // pgsql will either order them in a way that make it possible to execute,
    // or will make one fail.
    // See: https://www.postgresql.org/docs/current/transaction-iso.html#XACT-SERIALIZABLE
    //
    // If we were not doing this, we would have to acquire a lock for each proof, blind_signature
    // entry we read in db so that no other swap make use of them during this time.
    // I believe it's better to leave it to the db rather than manage it manualy.
    sqlx::query!("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;")
        .execute(&mut *tx)
        .await?;

    // First light (not io/computational intensive) verification
    // We:
    // - check they are not already used in db
    // - compute their respective total amount
    // This way we take won't do the rest of the job if those easy ones fail
    let outputs_total_amount = outputs_verification(&mut tx, &swap_request.outputs).await?;
    let inputs_total_amount = inputs_verification(&mut tx, &swap_request.inputs).await?;

    // Second round of verification and process
    // This is a bit more computational intensive
    // and requires to access the cache (which can trigger db reads and async waiting)
    let (fee, insert_spent_proofs_query_builder) = process_inputs(
        &mut tx,
        &mut keyset_cache,
        &keys_manager,
        &swap_request.inputs,
    )
    .await?;

    if inputs_total_amount
        != outputs_total_amount
            .checked_add(&fee.into())
            .ok_or(Error::Overflow)?
    {
        return Err(Error::TransactionUnbalanced(
            inputs_total_amount,
            outputs_total_amount,
            fee,
        ));
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

    Ok(Json(PostSwapResponse {
        signatures: blind_signatures,
    }))
}

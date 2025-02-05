use axum::{extract::State, Json};
use keys_manager::KeysManager;
use num_traits::CheckedAdd;
use nuts::{
    nut03::{SwapRequest, SwapResponse},
    Amount,
};
use sqlx::PgPool;

use crate::{
    errors::{Error, SwapError},
    keyset_cache::KeysetCache,
    logic::{process_outputs, process_outputs_allow_multiple_units, process_swap_inputs},
};

pub async fn swap(
    State(pool): State<PgPool>,
    State(mut keyset_cache): State<KeysetCache>,
    State(keys_manager): State<KeysManager>,
    Json(swap_request): Json<SwapRequest>,
) -> Result<Json<SwapResponse>, Error> {
    let mut tx = memory_db::start_db_tx(&pool).await?;

    let outputs_amounts =
        process_outputs_allow_multiple_units(&mut tx, &mut keyset_cache, &swap_request.outputs)
            .await?;

    // Second round of verification and process
    let (input_fees_and_amount, insert_spent_proofs_query_builder) = process_swap_inputs(
        &mut tx,
        &mut keyset_cache,
        &keys_manager,
        &swap_request.inputs,
    )
    .await?;

    // Amount matching
    for (asset, output_amount) in outputs_amounts {
        let &(_, fee, input_amount) = input_fees_and_amount
            .iter()
            .find(|(u, _, _)| *u == asset)
            .ok_or(SwapError::UnbalancedUnits)?;

        if input_amount
            != output_amount
                .checked_add(&Amount::from(fee))
                .ok_or(Error::Overflow)?
        {
            Err(SwapError::TransactionUnbalanced(
                asset,
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

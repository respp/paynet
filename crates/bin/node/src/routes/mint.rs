use axum::{
    extract::{Path, State},
    Json,
};
use keys_manager::KeysManager;
use nuts::nut04::{MintQuoteState, MintRequest, MintResponse};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    errors::{Error, MintError},
    keyset_cache::KeysetCache,
    logic::{process_outputs, process_outputs_allow_single_unit},
    methods::Method,
};

pub async fn mint(
    Path(method): Path<Method>,
    State(pool): State<PgPool>,
    State(mut keyset_cache): State<KeysetCache>,
    State(keys_manager): State<KeysManager>,
    Json(mint_request): Json<MintRequest<Uuid>>,
) -> Result<Json<MintResponse>, Error> {
    match method {
        Method::Starknet => {}
    }

    let mut tx = memory_db::start_db_tx(&pool).await?;

    let (expected_amount, state) =
        memory_db::mint_quote::get_amount_and_state(&mut tx, mint_request.quote).await?;

    if state != MintQuoteState::Paid {
        return Err(MintError::InvalidQuoteStateAtThisPoint(state).into());
    }

    let total_amount =
        process_outputs_allow_single_unit(&mut tx, &mut keyset_cache, &mint_request.outputs)
            .await?;

    if total_amount != expected_amount {
        return Err(MintError::UnbalancedMintAndQuoteAmounts(total_amount, expected_amount).into());
    }

    let (blind_signatures, insert_blind_signatures_query_builder) = process_outputs(
        &mut tx,
        &mut keyset_cache,
        &keys_manager,
        &mint_request.outputs,
    )
    .await?;

    insert_blind_signatures_query_builder
        .execute(&mut tx)
        .await?;
    memory_db::mint_quote::set_state(&mut tx, mint_request.quote, MintQuoteState::Issued).await?;

    tx.commit().await?;

    Ok(Json(MintResponse {
        signatures: blind_signatures,
    }))
}

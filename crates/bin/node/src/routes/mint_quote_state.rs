use axum::{
    extract::{Path, State},
    Json,
};
use nuts::nut04::MintQuoteResponse;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{errors::Error, methods::Method};

pub async fn mint_quote_state(
    Path((method, quote_id)): Path<(Method, Uuid)>,
    State(pool): State<PgPool>,
) -> Result<Json<MintQuoteResponse<Uuid>>, Error> {
    match method {
        Method::Starknet => {}
    }

    let mut conn = pool.acquire().await?;

    let mint_quote_response = memory_db::build_mint_quote_response(&mut conn, quote_id).await?;

    Ok(Json(mint_quote_response))
}

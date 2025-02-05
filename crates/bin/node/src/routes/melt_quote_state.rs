use axum::{
    extract::{Path, State},
    Json,
};
use nuts::nut05::MeltQuoteResponse;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{errors::Error, methods::Method};

pub async fn melt_quote_state(
    Path((method, quote_id)): Path<(Method, Uuid)>,
    State(pool): State<PgPool>,
) -> Result<Json<MeltQuoteResponse<Uuid>>, Error> {
    match method {
        Method::Starknet => {}
    }

    let mut conn = pool.acquire().await?;

    let melt_quote_response =
        memory_db::melt_quote::build_response_from_db(&mut conn, quote_id).await?;

    Ok(Json(melt_quote_response))
}

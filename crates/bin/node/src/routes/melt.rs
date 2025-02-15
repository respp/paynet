use axum::{
    extract::{Path, State},
    Json,
};
use num_traits::CheckedAdd;
use nuts::{
    nut05::{MeltQuoteResponse, MeltQuoteState, MeltRequest},
    Amount,
};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    app_state::SharedSignerClient,
    errors::{Error, MeltError},
    keyset_cache::KeysetCache,
    logic::process_melt_inputs,
    methods::Method,
    Unit,
};

async fn handle_unpaid_quote(
    tx: &mut Transaction<'_, Postgres>,
    signer: SharedSignerClient,
    keyset_cache: &mut KeysetCache,
    melt_request: &MeltRequest<Uuid>,
    quote_unit: Unit,
    expected_amount: Amount,
    fee_reserve: Amount,
) -> Result<(), Error> {
    let (total_amount, insert_spent_proofs_query_builder) =
        process_melt_inputs(tx, signer, keyset_cache, &melt_request.inputs, quote_unit).await?;

    let total_expected_amount = expected_amount
        .checked_add(&fee_reserve)
        .ok_or(Error::Overflow)?;
    if total_amount < total_expected_amount {
        return Err(
            MeltError::UnbalancedMeltAndQuoteAmounts(total_amount, total_expected_amount).into(),
        );
    }

    // All verifications done, melt the tokens, and update state
    insert_spent_proofs_query_builder.execute(tx).await?;
    memory_db::melt_quote::set_state(tx, melt_request.quote, MeltQuoteState::Pending).await?;

    Ok(())
}

pub async fn melt(
    Path(method): Path<Method>,
    State(pool): State<PgPool>,
    State(signer_client): State<SharedSignerClient>,
    State(mut keyset_cache): State<KeysetCache>,
    Json(melt_request): Json<MeltRequest<Uuid>>,
) -> Result<Json<MeltQuoteResponse<Uuid>>, Error> {
    match method {
        Method::Starknet => {}
    }

    let mut conn = pool.acquire().await?;
    let mut tx = memory_db::start_db_tx_from_conn(&mut conn).await?;

    let (quote_unit, expected_amount, fee_reserve, mut state, expiry) =
        memory_db::melt_quote::get_data::<Unit>(&mut tx, melt_request.quote).await?;

    match state {
        MeltQuoteState::Unpaid => {
            handle_unpaid_quote(
                &mut tx,
                signer_client,
                &mut keyset_cache,
                &melt_request,
                quote_unit,
                expected_amount,
                fee_reserve,
            )
            .await?;
            state = MeltQuoteState::Pending
        }
        MeltQuoteState::Pending => {}
        MeltQuoteState::Paid => return Err(MeltError::InvalidQuoteStateAtThisPoint(state).into()),
    }
    tx.commit().await?;

    match state {
        MeltQuoteState::Unpaid => {
            return Err(MeltError::InvalidQuoteStateAtThisPoint(state).into());
        }
        MeltQuoteState::Pending => proceed_to_payment().await?,
        MeltQuoteState::Paid => {}
    }

    // State should be paid at this point
    let state = memory_db::melt_quote::get_state(&mut conn, melt_request.quote).await?;
    if state != MeltQuoteState::Paid {
        return Err(MeltError::InvalidQuoteStateAtThisPoint(state).into());
    }

    Ok(Json(MeltQuoteResponse {
        quote: melt_request.quote,
        amount: expected_amount,
        fee_reserve,
        state: MeltQuoteState::Paid,
        expiry,
    }))
}

#[cfg(feature = "uncollateralized")]
async fn proceed_to_payment() -> Result<(), Error> {
    Ok(())
}
#[cfg(not(feature = "uncollateralized"))]
async fn proceed_to_payment() -> Result<(), Error> {
    // TODO: actually proceed to payment
    Ok(())
}

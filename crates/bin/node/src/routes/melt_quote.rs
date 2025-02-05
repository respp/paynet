use axum::{
    extract::{Path, State},
    Json,
};
use cashu_starknet::{MeltPaymentRequest, StarknetU256};
use num_traits::CheckedAdd;
use nuts::{
    nut05::{MeltMethodSettings, MeltQuoteRequest, MeltQuoteResponse, MeltQuoteState},
    Amount,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    app_state::{ArcQuoteTTLConfigState, NutsSettingsState},
    errors::{Error, MeltError, QuoteError},
    methods::Method,
    utils::unix_time,
    Unit,
};

pub async fn melt_quote(
    Path(method): Path<Method>,
    State(pool): State<PgPool>,
    State(nuts): State<NutsSettingsState>,
    State(quote_ttl): State<ArcQuoteTTLConfigState>,
    Json(melt_quote_request): Json<MeltQuoteRequest<Unit>>,
) -> Result<Json<MeltQuoteResponse<Uuid>>, Error> {
    // Release the lock asap
    let settings = {
        let read_nuts_settings_lock = nuts.read();

        if read_nuts_settings_lock.nut05.disabled {
            Err(QuoteError::MeltDisabled)?;
        }

        read_nuts_settings_lock
            .nut05
            .get_settings(method, melt_quote_request.unit)
            .ok_or(QuoteError::UnitNotSupported(
                melt_quote_request.unit,
                method,
            ))?
    };

    let response = match method {
        Method::Starknet => {
            handle_starknet_melt_quote(pool, settings, melt_quote_request, quote_ttl.melt_ttl())
                .await?
        }
    };

    Ok(Json(response))
}

async fn handle_starknet_melt_quote(
    pool: PgPool,
    settings: MeltMethodSettings<Method, Unit>,
    melt_quote_request: MeltQuoteRequest<Unit>,
    mint_ttl: u64,
) -> Result<MeltQuoteResponse<Uuid>, Error> {
    let payment_request: MeltPaymentRequest = serde_json::from_str(&melt_quote_request.request)?;

    if !melt_quote_request
        .unit
        .is_asset_supported(payment_request.asset)
    {
        return Err(
            MeltError::InvalidAssetForUnit(payment_request.asset, melt_quote_request.unit).into(),
        );
    }

    let (required_amount, remainder) = melt_quote_request
        .unit
        .convert_u256_into_amount(payment_request.amount)?;
    if remainder != StarknetU256::ZERO {
        required_amount
            .checked_add(&Amount::ONE)
            .ok_or(Error::Overflow)?;
    }

    if let Some(min_amount) = settings.min_amount {
        if min_amount > required_amount {
            Err(QuoteError::AmountTooLow(min_amount, required_amount))?;
        }
    }
    if let Some(max_amount) = settings.max_amount {
        if max_amount < required_amount {
            Err(QuoteError::AmountTooHigh(max_amount, required_amount))?;
        }
    }

    let expiry = unix_time() + mint_ttl;
    let quote = Uuid::new_v4();
    // Arbitrary, but will be enough to pay tx fee on starkent
    // TODO: Optimize by setting to zero when remainder is already enough to pay
    let fee_reserve = Amount::ONE;

    let mut conn = pool.acquire().await?;
    memory_db::melt_quote::insert_new(
        &mut conn,
        quote,
        melt_quote_request.unit,
        required_amount,
        fee_reserve,
        &melt_quote_request.request,
        expiry,
    )
    .await?;

    Ok(MeltQuoteResponse {
        quote,
        amount: required_amount,
        fee_reserve,
        state: MeltQuoteState::Unpaid,
        expiry,
    })
}

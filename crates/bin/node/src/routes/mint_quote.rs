use crate::errors::{Error, MintError, QuoteError};
use axum::{
    extract::{Path, State},
    Json,
};
use cashu_starknet::{MintPaymentRequest, PayInvoiceCalldata, STRK_TOKEN_ADDRESS};
use nuts::{
    nut04::{MintQuoteRequest, MintQuoteResponse},
    QuoteState,
};
use sqlx::PgPool;
use starknet_types_core::felt::Felt;
use uuid::Uuid;

use crate::{
    app_state::{ArcQuoteTTLConfigState, NutsSettingsState},
    methods::Method,
    utils::unix_time,
    Unit,
};

pub async fn mint_quote(
    Path(method): Path<Method>,
    State(pool): State<PgPool>,
    State(nuts): State<NutsSettingsState>,
    State(quote_ttl): State<ArcQuoteTTLConfigState>,
    Json(mint_quote_request): Json<MintQuoteRequest<Unit>>,
) -> Result<Json<MintQuoteResponse<Uuid>>, Error> {
    // Release the lock asap
    let settings = {
        let read_nuts_settings_lock = nuts.read();

        if read_nuts_settings_lock.nut04.disabled {
            Err(QuoteError::MintDisabled)?;
        }

        read_nuts_settings_lock
            .nut04
            .get_settings(method, mint_quote_request.unit)
            .ok_or(QuoteError::UnitNotSupported(
                mint_quote_request.unit,
                method,
            ))?
    };

    if let Some(min_amount) = settings.min_amount {
        if min_amount > mint_quote_request.amount {
            Err(QuoteError::AmountTooLow(
                min_amount,
                mint_quote_request.amount,
            ))?;
        }
    }
    if let Some(max_amount) = settings.max_amount {
        if max_amount < mint_quote_request.amount {
            Err(QuoteError::AmountTooHigh(
                max_amount,
                mint_quote_request.amount,
            ))?;
        }
    }

    if mint_quote_request.description.is_some() && !settings.description {
        Err(MintError::DescriptionNotSupported)?;
    }

    let response = match method {
        Method::Starknet => new_starknet_mint_quote(pool, mint_quote_request, quote_ttl.mint_ttl()),
    }
    .await?;

    Ok(Json(response))
}

/// Initialize a new Starknet mint quote
async fn new_starknet_mint_quote(
    pool: PgPool,
    mint_quote_request: MintQuoteRequest<Unit>,
    mint_ttl: u64,
) -> Result<MintQuoteResponse<Uuid>, Error> {
    let expiry = unix_time() + mint_ttl;
    let quote = Uuid::new_v4();

    let asset = match mint_quote_request.unit {
        Unit::Strk | Unit::StrkAtto => STRK_TOKEN_ADDRESS,
    };
    let amount = mint_quote_request
        .unit
        .convert_amount_into_u256(mint_quote_request.amount);

    let request = serde_json::to_string(&MintPaymentRequest {
        contract_address: Felt::from_hex_unchecked(
            "0x03a94f47433e77630f288054330fb41377ffcc49dacf56568eeba84b017aa633",
        ),
        selector: Felt::from_hex_unchecked(
            "0x027a12f554d018764f982295090da45b4ff0734785be0982b62c329b9ac38033",
        ),
        calldata: PayInvoiceCalldata {
            invoice_id: quote.as_u128(),
            asset,
            amount,
        },
    })?;

    let mut conn = pool.acquire().await?;
    memory_db::insert_new_mint_quote(
        &mut conn,
        quote,
        mint_quote_request.unit,
        mint_quote_request.amount,
        &request,
        expiry,
    )
    .await?;

    Ok(MintQuoteResponse {
        quote,
        request,
        state: QuoteState::Unpaid,
        expiry,
    })
}

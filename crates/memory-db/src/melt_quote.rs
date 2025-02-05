use nuts::{
    nut05::{MeltQuoteResponse, MeltQuoteState},
    traits::Unit,
    Amount,
};
use sqlx::{types::time::OffsetDateTime, PgConnection};
use uuid::Uuid;

use crate::Error;

pub async fn insert_new<U: Unit>(
    conn: &mut PgConnection,
    quote_id: Uuid,
    unit: U,
    amount: Amount,
    fee: Amount,
    request: &str,
    expiry: u64,
) -> Result<(), Error> {
    let expiry: i64 = expiry
        .try_into()
        .map_err(|_| Error::RuntimeToDbConversion)?;
    let expiry =
        OffsetDateTime::from_unix_timestamp(expiry).map_err(|_| Error::RuntimeToDbConversion)?;

    sqlx::query!(
        r#"INSERT INTO melt_quote (id, unit, amount, fee_reserve, request, expiry, state) VALUES ($1, $2, $3, $4, $5, $6, 0)"#,
        quote_id,
        &unit.to_string(),
        amount.into_i64_repr(),
        fee.into_i64_repr(),
        request,
        expiry,
    ).execute(conn).await?;

    Ok(())
}

pub async fn build_response_from_db(
    conn: &mut PgConnection,
    quote_id: Uuid,
) -> Result<MeltQuoteResponse<Uuid>, Error> {
    let record = sqlx::query!(
        r#"SELECT amount, fee_reserve, state, expiry  FROM melt_quote where id = $1"#,
        quote_id
    )
    .fetch_one(conn)
    .await?;

    let state = record
        .state
        .try_into()
        .map_err(|_| Error::DbToRuntimeConversion)?;
    let expiry = record
        .expiry
        .unix_timestamp()
        .try_into()
        .map_err(|_| Error::DbToRuntimeConversion)?;
    let amount = Amount::from_i64_repr(record.amount);
    let fee_reserve = Amount::from_i64_repr(record.fee_reserve);

    Ok(MeltQuoteResponse {
        quote: quote_id,
        amount,
        fee_reserve,
        state,
        expiry,
    })
}

pub async fn get_data<U: Unit>(
    conn: &mut PgConnection,
    quote_id: Uuid,
) -> Result<(U, Amount, Amount, MeltQuoteState, u64), Error> {
    let record = sqlx::query!(
        r#"SELECT unit, amount, fee_reserve, state, expiry FROM melt_quote where id = $1"#,
        quote_id
    )
    .fetch_one(conn)
    .await?;

    let unit = U::from_str(&record.unit).map_err(|_| Error::DbToRuntimeConversion)?;
    let amount = Amount::from_i64_repr(record.amount);
    let fee_reserve = Amount::from_i64_repr(record.fee_reserve);
    let state = MeltQuoteState::try_from(record.state).map_err(|_| Error::DbToRuntimeConversion)?;
    let expiry = record
        .expiry
        .unix_timestamp()
        .try_into()
        .map_err(|_| Error::DbToRuntimeConversion)?;

    Ok((unit, amount, fee_reserve, state, expiry))
}

pub async fn get_state(conn: &mut PgConnection, quote_id: Uuid) -> Result<MeltQuoteState, Error> {
    let record = sqlx::query!(r#"SELECT state FROM melt_quote where id = $1"#, quote_id)
        .fetch_one(conn)
        .await?;

    let state = MeltQuoteState::try_from(record.state).map_err(|_| Error::DbToRuntimeConversion)?;

    Ok(state)
}

pub async fn set_state(
    conn: &mut PgConnection,
    quote_id: Uuid,
    state: MeltQuoteState,
) -> Result<(), Error> {
    sqlx::query!(
        r#"
            UPDATE melt_quote
            SET state = $2
            WHERE id = $1
        "#,
        quote_id,
        i16::from(state)
    )
    .fetch_one(conn)
    .await?;

    Ok(())
}

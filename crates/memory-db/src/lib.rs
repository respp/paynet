use std::str::FromStr;

use nuts::{
    nut01::PublicKey, nut02::KeysetId, nut04::MintQuoteResponse, traits::Unit, Amount, QuoteState,
};
use sqlx::{types::time::OffsetDateTime, PgConnection};
use thiserror::Error;

mod insert_spent_proofs;
pub use insert_spent_proofs::InsertSpentProofsQueryBuilder;
mod insert_blind_signatures;
pub use insert_blind_signatures::InsertBlindSignaturesQueryBuilder;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to acquire lock")]
    Lock,
    #[error("Failed to compute y by running hash_on_curve")]
    HashOnCurve,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("Failed to convert the unit db record to the passed generic Unit type: \"{0}\"")]
    InvalidUnit(String),
    #[error("Failed to convert the db type into the runtime type")]
    DbToRuntimeConversion,
    #[error("Failed to convert the runtime type into the db type")]
    RuntimeToDbConversion,
}

#[derive(Debug, Clone)]
pub struct KeysetInfo<U> {
    unit: U,
    active: bool,
    max_order: u8,
    derivation_path_index: u32,
    input_fee_ppk: u16,
}

impl<U: Unit> KeysetInfo<U> {
    pub fn unit(&self) -> U {
        self.unit
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn max_order(&self) -> u8 {
        self.max_order
    }
    pub fn derivation_path_index(&self) -> u32 {
        self.derivation_path_index
    }
    pub fn input_fee_ppk(&self) -> u16 {
        self.input_fee_ppk
    }
}

pub async fn get_keyset<U: FromStr>(
    conn: &mut PgConnection,
    keyset_id: &KeysetId,
) -> Result<KeysetInfo<U>, Error> {
    let record = sqlx::query!(
        r#"SELECT unit, active, max_order, derivation_path_index, input_fee_ppk
        FROM keyset
        WHERE id = $1"#,
        keyset_id.as_i64()
    )
    .fetch_one(conn)
    .await?;

    let info = KeysetInfo {
        unit: U::from_str(&record.unit).map_err(|_| Error::InvalidUnit(record.unit))?,
        active: record.active,
        max_order: u8::try_from(record.max_order).map_err(|_| Error::DbToRuntimeConversion)?,
        derivation_path_index: u32::from_be_bytes(record.derivation_path_index.to_be_bytes()),
        input_fee_ppk: u16::from_be_bytes(record.input_fee_ppk.to_be_bytes()),
    };

    Ok(info)
}

/// Will return true if this secret has already been signed by us
pub async fn is_any_blind_message_already_used(
    conn: &mut PgConnection,
    blind_secrets: impl Iterator<Item = PublicKey>,
) -> Result<bool, Error> {
    let ys: Vec<_> = blind_secrets.map(|pk| pk.to_bytes().to_vec()).collect();

    let record = sqlx::query!(
        r#"SELECT EXISTS (
            SELECT * FROM blind_signature WHERE y = ANY($1)
        ) AS "exists!";"#,
        &ys
    )
    .fetch_one(conn)
    .await?;

    Ok(record.exists)
}

/// Will return true if one of the provided secret
/// is already in db with state = SPENT
pub async fn is_any_proof_already_used(
    conn: &mut PgConnection,
    secret_derived_pubkeys: impl Iterator<Item = PublicKey>,
) -> Result<bool, Error> {
    let ys: Vec<_> = secret_derived_pubkeys
        .map(|pk| pk.to_bytes().to_vec())
        .collect();

    let record = sqlx::query!(
        r#"SELECT EXISTS (
            SELECT * FROM proof WHERE y = ANY($1) AND state = 1
        ) AS "exists!";"#,
        &ys
    )
    .fetch_one(conn)
    .await?;

    Ok(record.exists)
}

pub async fn get_keyset_input_fee(
    conn: &mut PgConnection,
    keyset_id: &KeysetId,
) -> Result<u16, Error> {
    let keyset_id = keyset_id.as_i64();

    let record = sqlx::query!(
        r#"SELECT input_fee_ppk FROM keyset where id = $1"#,
        keyset_id
    )
    .fetch_one(conn)
    .await?;

    // pgsql doesn't support unsigned numbers so we cast them as signed before storing,
    // and to the oposite when reading
    let input_fee_ppk = u16::from_be_bytes(record.input_fee_ppk.to_be_bytes());

    Ok(input_fee_ppk)
}

pub async fn insert_new_mint_quote<U: Unit>(
    conn: &mut PgConnection,
    quote_id: Uuid,
    unit: U,
    amount: Amount,
    request: &str,
    expiry: u64,
) -> Result<(), Error> {
    let expiry: i64 = expiry
        .try_into()
        .map_err(|_| Error::RuntimeToDbConversion)?;
    let expiry =
        OffsetDateTime::from_unix_timestamp(expiry).map_err(|_| Error::RuntimeToDbConversion)?;

    sqlx::query!(
        r#"INSERT INTO mint_quote (id, unit, amount, request, expiry, state) VALUES ($1, $2, $3, $4, $5, 0)"#,
        quote_id,
        &unit.to_string(),
        amount.into_i64_repr(),
        request,
        expiry,
    ).execute(conn).await?;

    Ok(())
}

pub async fn insert_new_melt_quote<U: Unit>(
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

pub async fn build_mint_quote_response(
    conn: &mut PgConnection,
    quote_id: Uuid,
) -> Result<MintQuoteResponse<Uuid>, Error> {
    let record = sqlx::query!(
        r#"SELECT request, state, expiry  FROM mint_quote where id = $1"#,
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

    Ok(MintQuoteResponse {
        quote: quote_id,
        request: record.request,
        state,
        expiry,
    })
}

pub async fn get_amount_and_state_for_mint_quote_by_id(
    conn: &mut PgConnection,
    quote_id: Uuid,
) -> Result<(Amount, QuoteState), Error> {
    let record = sqlx::query!(
        r#"SELECT amount, state FROM mint_quote where id = $1"#,
        quote_id
    )
    .fetch_one(conn)
    .await?;

    let amount = Amount::from_i64_repr(record.amount);
    let state = QuoteState::try_from(record.state).map_err(|_| Error::DbToRuntimeConversion)?;

    Ok((amount, state))
}

/// Handle concurency at the database level
/// If one transaction alter a field that is used in another one
/// in a way that would result in a different statement output,
/// pgsql will either order them in a way that make it possible to execute,
/// or will make one fail.
/// See: https://www.postgresql.org/docs/current/transaction-iso.html#XACT-SERIALIZABLE
///
/// To be use at the very begining of a transaction.
///
/// If we were not doing this, we would have to acquire a lock for each proof, blind_signature
/// entry we read in db so that no other swap make use of them during this time.
/// I believe it's better to leave it to the db rather than manage it manualy.
pub async fn set_transaction_isolation_level_to_serializable(
    conn: &mut PgConnection,
) -> Result<(), Error> {
    sqlx::query!("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;")
        .execute(conn)
        .await?;

    Ok(())
}

pub async fn sum_amount_of_unit_in_circulation<U: Unit>(
    conn: &mut PgConnection,
    unit: U,
) -> Result<Amount, Error> {
    let record = sqlx::query!(
        r#"
            SELECT SUM(amount) AS "sum!: i64" FROM blind_signature 
            INNER JOIN keyset ON blind_signature.keyset_id = keyset.id
            WHERE keyset.unit = $1;
        "#,
        &unit.to_string()
    )
    .fetch_one(conn)
    .await?;

    let amount = Amount::from_i64_repr(record.sum);

    Ok(amount)
}

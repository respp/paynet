use std::str::FromStr;

use nuts::{nut01::PublicKey, nut02::KeysetId, traits::Unit};
use sqlx::PgConnection;
use thiserror::Error;

mod insert_spent_proofs;
pub use insert_spent_proofs::InsertSpentProofsQueryBuilder;
mod insert_blind_signatures;
pub use insert_blind_signatures::InsertBlindSignaturesQueryBuilder;

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

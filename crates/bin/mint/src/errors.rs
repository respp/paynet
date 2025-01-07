use axum::{http::StatusCode, response::IntoResponse, Json};
use nuts::{dhke, nut00::CashuError, Amount};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Dhke(#[from] dhke::Error),
    /// Something went wrong in the db
    #[error(transparent)]
    Database(#[from] memory_db::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    /// BlindMessage is already signed
    #[error("Blind Message is already signed")]
    BlindMessageAlreadySigned,
    #[error("Keyset doesn't exist in this mint")]
    UnknownKeySet,
    #[error("No keypair for amount")]
    InvalidAmountKey,
    #[error("A value overflowed during execution")]
    Overflow,
    /// Transaction unbalanced
    #[error("Inputs: `{0}`, Outputs: `{1}`, Expected Fee: `{2}`")]
    TransactionUnbalanced(Amount, Amount, u16),
    #[error("The KeyManager generated a KeysetId different from the one known in db")]
    GeneratedKeysetIdIsDifferentFromOriginal,
    /// Multiple units provided
    #[error("Cannot have multiple units")]
    MultipleUnits,
    /// Inactive Keyset
    #[error("Inactive Keyset")]
    InactiveKeyset,
    #[error("Duplicate output")]
    DuplicateOutput,
    #[error("Failed to compute y by running hash_on_curve")]
    HashOnCurve,
}

impl From<Error> for CashuError {
    fn from(_value: Error) -> Self {
        Self::new(0, String::new())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::BAD_REQUEST, Json(CashuError::from(self))).into_response()
    }
}

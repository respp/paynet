use axum::{http::StatusCode, response::IntoResponse, Json};
use cashu_starknet::{Asset, StarknetU256};
use nuts::{dhke, nut00::CashuError, nut04::MintQuoteState, nut05::MeltQuoteState, Amount};
use thiserror::Error;

use crate::{methods::Method, Unit};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Dhke(#[from] dhke::Error),
    #[error(transparent)]
    Database(#[from] memory_db::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Mint(#[from] MintError),
    #[error(transparent)]
    Swap(#[from] SwapError),
    #[error(transparent)]
    Quote(#[from] QuoteError),
    #[error(transparent)]
    Melt(#[from] MeltError),
    #[error(transparent)]
    Starknet(#[from] StarknetError),
    #[error("Keyset doesn't exist in this mint")]
    UnknownKeySet,
    #[error("No keypair for amount")]
    InvalidAmountKey,
    #[error("A value overflowed during execution")]
    Overflow,
    #[error("The KeyManager generated a KeysetId different from the one known in db")]
    GeneratedKeysetIdIsDifferentFromOriginal,
    /// Inactive Keyset
    #[error("Inactive Keyset")]
    InactiveKeyset,
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

#[derive(Debug, Error)]
pub enum SwapError {
    /// BlindMessage is already signed
    #[error("Blind Message is already signed")]
    BlindMessageAlreadySigned,
    #[error("All input units should be present as output")]
    UnbalancedUnits,
    /// Transaction unbalanced
    #[error("For asset {0}, Inputs: `{1}`, Outputs: `{2}`, Expected Fee: `{3}`")]
    TransactionUnbalanced(Unit, Amount, Amount, u16),
    #[error("Duplicate input")]
    DuplicateInput,
    #[error("Duplicate output")]
    DuplicateOutput,
}

#[derive(Debug, Error)]
pub enum MintError {
    #[error("Method does not support description field")]
    DescriptionNotSupported,
    #[error("Mint request amounts sum is {0} for a quote of {1}")]
    UnbalancedMintAndQuoteAmounts(Amount, Amount),
    #[error("Invalid quote state {0} at this poin of the flow")]
    InvalidQuoteStateAtThisPoint(MintQuoteState),
}

#[derive(Debug, Error)]
pub enum QuoteError {
    #[error("Minting is currently disabled")]
    MintDisabled,
    #[error("Melting is currently disabled")]
    MeltDisabled,
    #[error("This quote require the use of multiple units")]
    MultipleUnits,
    #[error("Unsupported unit `{0}` for method `{1}`")]
    UnitNotSupported(Unit, Method),
    #[error("Amount must be at least {0}, got {1}")]
    AmountTooLow(Amount, Amount),
    #[error("Amount must bellow {0}, got {1}")]
    AmountTooHigh(Amount, Amount),
    #[error("This quote has already been issued")]
    IssuedQuote,
}

#[derive(Debug, Error)]
pub enum MeltError {
    #[error("Asset {0} is not supported by unit {1}")]
    InvalidAssetForUnit(Asset, Unit),
    #[error("Quote specifed {0}, inputs provided {1}")]
    InvalidInputsUnit(Unit, Unit),
    #[error("Melt request amounts sum is {0} for a quote of {1}")]
    UnbalancedMeltAndQuoteAmounts(Amount, Amount),
    #[error("Invalid quote state {0} at this point of the flow")]
    InvalidQuoteStateAtThisPoint(MeltQuoteState),
}

#[derive(Debug, Error)]
pub enum StarknetError {
    #[error(
        "Starknet u256 amount of {1} is to big to be converted into a cashu Amount for unit {0}"
    )]
    StarknetAmountTooHigh(Unit, StarknetU256),
}

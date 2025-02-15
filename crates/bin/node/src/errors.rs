use axum::{http::StatusCode, response::IntoResponse, Json};
use cashu_starknet::{Asset, Unit};
use nuts::{
    dhke, nut00::CashuError, nut01, nut02, nut04::MintQuoteState, nut05::MeltQuoteState, Amount,
};
use thiserror::Error;

use crate::{commands::ConfigError, methods::Method};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Init(#[from] InitializationError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    Dhke(#[from] dhke::Error),
    #[error(transparent)]
    Nut02(#[from] nut02::Error),
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
    Proof(#[from] ProofError),
    #[error(transparent)]
    BlindMessage(#[from] BlindMessageError),
    #[error(transparent)]
    Starknet(#[from] cashu_starknet::Error),
    #[error(transparent)]
    Tonic(#[from] tonic::Status),
    #[error(transparent)]
    Signer(#[from] SignerError),
    #[error(transparent)]
    Service(#[from] ServiceError),
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
pub enum ProofError {
    #[error("Invalid Proof")]
    Invalid,
    #[error("Proof already used")]
    Used,
}

#[derive(Debug, Error)]
pub enum BlindMessageError {
    #[error("Blind message is already signed")]
    AlreadySigned,
}

#[derive(Debug, Error)]
pub enum SwapError {
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
pub enum InitializationError {
    #[error("Failed to read the config file: {0}")]
    CannotReadConfig(#[source] std::io::Error),
    #[error("Failed to deserialize the config file as toml: {0}")]
    Toml(#[source] toml::de::Error),
    #[error("Failed to connect to database: {0}")]
    DbConnect(#[source] sqlx::Error),
    #[error("Failed to run the database migration: {0}")]
    DbMigrate(#[source] sqlx::migrate::MigrateError),
    #[cfg(debug_assertions)]
    #[error("Failed to load .env file: {0}")]
    Dotenvy(#[source] dotenvy::Error),
    #[error("Failed to read variable from environment: {0}")]
    Env(#[source] std::env::VarError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("Failed init apibara indexer: {0}")]
    InitIndexer(#[source] invoice_payment_indexer::Error),
    #[error("Failed bind tcp listener: {0}")]
    BindTcp(#[source] std::io::Error),
    #[error("Failed to open the SqLite db: {0}")]
    OpenSqlite(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Failed to await on indexer future: {0}")]
    JoinIndexer(#[source] tokio::task::JoinError),
    #[error("Failed to run the indexer: {0}")]
    Indexer(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("Failed to serve the axum server: {0}")]
    AxumServe(#[source] std::io::Error),
}

#[derive(Debug, Error)]
pub enum SignerError {
    #[error("invalid bytes C: {0}")]
    BlindSignature(#[from] nut01::Error),
    #[error("failed to connect")]
    Connection(#[from] tonic::transport::Error),
}

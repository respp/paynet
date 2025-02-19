use cashu_starknet::{Asset, Unit};
use nuts::Amount;
use tonic::Status;

use crate::{logic::InputsError, methods::Method};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to commit db tx: {0}")]
    TxCommit(#[source] sqlx::Error),
    #[error("failed to commit db tx: {0}")]
    TxBegin(#[source] sqlx::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Db(#[from] memory_db::Error),
    #[error("failed to serialize the request content")]
    MeltDisabled,
    #[error("Unsupported unit `{0}` for method `{1}`")]
    UnitNotSupported(Unit, Method),
    #[error("Unsupported asset `{0}` for unit `{1}`")]
    InvalidAssetForUnit(Asset, Unit),
    #[error("the sum off all the inputs' amount must fit in a u64")]
    TotalAmountTooBig,
    #[error(transparent)]
    Inputs(#[from] InputsError),
    #[error("total inputs's amount {0} is lower than the node minimal amount {1} ")]
    AmountTooLow(Amount, Amount),
    #[error("total inputs's amount {0} is higher than the node maximal amount {1} ")]
    AmountTooHigh(Amount, Amount),
}

impl From<Error> for Status {
    fn from(value: Error) -> Self {
        match value {
            Error::TxBegin(error) | Error::TxCommit(error) | Error::Sqlx(error) => {
                Status::internal(error.to_string())
            }
            Error::MeltDisabled => Status::failed_precondition(value.to_string()),
            Error::UnitNotSupported(_, _)
            | Error::InvalidAssetForUnit(_, _)
            | Error::AmountTooLow(_, _)
            | Error::AmountTooHigh(_, _)
            | Error::TotalAmountTooBig => Status::invalid_argument(value.to_string()),
            Error::Inputs(error) => error.into(),
            Error::Db(error) => Status::internal(error.to_string()),
        }
    }
}

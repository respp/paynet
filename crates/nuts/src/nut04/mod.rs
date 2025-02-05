use serde::{Deserialize, Serialize};

use crate::{
    nut00::{BlindSignature, BlindedMessage},
    traits::Unit,
    Amount, InvalidValueForQuoteState,
};

#[derive(
    Debug, Clone, Copy, Hash, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum MintQuoteState {
    /// Quote has not been paid
    #[default]
    Unpaid,
    /// Quote has been paid and wallet can mint
    Paid,
    /// ecash issued for quote
    Issued,
}

impl core::fmt::Display for MintQuoteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                MintQuoteState::Unpaid => "UNPAID",
                MintQuoteState::Paid => "PAID",
                MintQuoteState::Issued => "ISSUED",
            }
        )
    }
}

impl TryFrom<i16> for MintQuoteState {
    type Error = InvalidValueForQuoteState;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MintQuoteState::Unpaid),
            1 => Ok(MintQuoteState::Paid),
            2 => Ok(MintQuoteState::Issued),
            _ => Err(InvalidValueForQuoteState),
        }
    }
}

impl From<MintQuoteState> for i16 {
    fn from(value: MintQuoteState) -> Self {
        match value {
            MintQuoteState::Unpaid => 0,
            MintQuoteState::Paid => 1,
            MintQuoteState::Issued => 2,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintQuoteRequest<U: Unit> {
    pub amount: Amount,
    pub unit: U,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintQuoteResponse<Q> {
    pub quote: Q,
    pub request: String,
    pub state: MintQuoteState,
    pub expiry: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintRequest<Q> {
    pub quote: Q,
    pub outputs: Vec<BlindedMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MintResponse {
    pub signatures: Vec<BlindSignature>,
}

/// Mint Method Settings
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MintMethodSettings<M, U> {
    /// Payment Method e.g. Starknet
    pub method: M,
    /// Currency Unit e.g. strk
    pub unit: U,
    /// Min Amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_amount: Option<Amount>,
    /// Max Amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_amount: Option<Amount>,
    /// Quote Description
    #[serde(default)]
    pub description: bool,
}

/// Mint Settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings<M, U> {
    /// Methods to mint
    pub methods: Vec<MintMethodSettings<M, U>>,
    /// Minting disabled
    pub disabled: bool,
}

impl<M, U> Settings<M, U>
where
    M: PartialEq + Eq + Clone,
    U: PartialEq + Eq + Clone,
{
    pub fn get_settings(&self, method: M, unit: U) -> Option<MintMethodSettings<M, U>> {
        self.methods
            .iter()
            .find(|&s| method == s.method && unit == s.unit)
            .cloned()
    }
}

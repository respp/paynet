use serde::{Deserialize, Serialize};

use crate::nut00::{BlindMessage, BlindSignature, Proofs};

#[derive(Debug, Serialize, Deserialize)]
pub struct PostSwapResponse {
    pub signatures: Vec<BlindSignature>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostSwapRequest {
    pub inputs: Proofs,
    pub outputs: Vec<BlindMessage>,
}

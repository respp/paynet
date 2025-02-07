//! NUT-19: Cached Responses
//!
//! <https://github.com/cashubtc/nuts/blob/main/19.md>

use serde::{Deserialize, Serialize};

/// Mint settings
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Settings {
    /// Number of seconds the responses are cached for
    pub ttl: Option<u64>,
    /// Cached endpoints
    pub cached_endpoints: Vec<CachedEndpoint>,
}

/// List of the methods and paths for which caching is enabled
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CachedEndpoint {
    /// HTTP Method
    pub method: Method,
    /// Route path
    pub path: Path,
}

impl CachedEndpoint {
    /// Create [`CachedEndpoint`]
    pub fn new(method: Method, path: Path) -> Self {
        Self { method, path }
    }
}

/// HTTP method
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Method {
    /// Get
    Get,
    /// POST
    Post,
}

/// Route path
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "&'static str")]
pub enum Path {
    /// Bolt11 Mint
    MintStarknet,
    /// Bolt11 Melt
    MeltStarknet,
    /// Swap
    Swap,
}

pub const V1_MINT_STARKNET: &str = "/v1/mint/starknet";
pub const V1_SWAP: &str = "/v1/swap";
pub const V1_MELT_STARKNET: &str = "/v1/melt/starknet";

impl From<Path> for &'static str {
    fn from(value: Path) -> Self {
        match value {
            Path::MintStarknet => V1_MINT_STARKNET,
            Path::MeltStarknet => V1_MELT_STARKNET,
            Path::Swap => V1_SWAP,
        }
    }
}

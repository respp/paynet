use std::sync::{atomic::AtomicU64, Arc};

use axum::extract::FromRef;
use keys_manager::KeysManager;
use nuts::{nut06::NutsSettings, QuoteTTLConfig};
use parking_lot::RwLock;
use sqlx::PgPool;

use crate::{keyset_cache::KeysetCache, methods::Method, Unit};

pub type NutsSettingsState = Arc<RwLock<NutsSettings<Method, Unit>>>;
pub type ArcQuoteTTLConfigState = Arc<QuoteTTLConfigState>;

// the application state
#[derive(Debug, Clone, FromRef)]
pub struct AppState {
    pg_pool: PgPool,
    keys_manager: KeysManager,
    keyset_cache: KeysetCache,
    nuts: NutsSettingsState,
    quote_ttl: Arc<QuoteTTLConfigState>,
}

impl AppState {
    pub fn new(
        pg_pool: PgPool,
        seed: &[u8],
        nuts_settings: NutsSettings<Method, Unit>,
        quote_ttl: QuoteTTLConfig,
    ) -> Self {
        Self {
            pg_pool,
            keys_manager: KeysManager::new(seed),
            keyset_cache: Default::default(),
            nuts: Arc::new(RwLock::new(nuts_settings)),
            quote_ttl: Arc::new(quote_ttl.into()),
        }
    }
}

#[derive(Debug)]
pub struct QuoteTTLConfigState {
    mint_ttl: AtomicU64,
    melt_ttl: AtomicU64,
}

impl QuoteTTLConfigState {
    pub fn mint_ttl(&self) -> u64 {
        self.mint_ttl.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub fn melt_ttl(&self) -> u64 {
        self.melt_ttl.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl From<QuoteTTLConfig> for QuoteTTLConfigState {
    fn from(value: QuoteTTLConfig) -> Self {
        Self {
            mint_ttl: value.mint_ttl.into(),
            melt_ttl: value.melt_ttl.into(),
        }
    }
}

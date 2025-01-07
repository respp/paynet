use axum::extract::FromRef;
use keys_manager::KeysManager;
use sqlx::PgPool;

use crate::keyset_cache::KeysetCache;

// the application state
#[derive(Clone, FromRef)]
pub struct AppState {
    pg_pool: PgPool,
    keys_manager: KeysManager,
    keyset_cache: KeysetCache,
}

impl AppState {
    pub fn new(pg_pool: PgPool, seed: &[u8]) -> Self {
        Self {
            pg_pool,
            keys_manager: KeysManager::new(seed),
            keyset_cache: Default::default(),
        }
    }
}

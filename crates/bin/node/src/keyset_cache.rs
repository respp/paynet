use std::{collections::HashMap, sync::Arc};

use cashu_starknet::Unit;
use nuts::nut02::KeysetId;
use parking_lot::RwLock;
use sqlx::PgConnection;

use crate::errors::Error;

#[derive(Debug, Clone)]
pub struct CachedKeysetInfo {
    active: bool,
    unit: Unit,
    input_fee_ppk: u16,
}

impl CachedKeysetInfo {
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn unit(&self) -> Unit {
        self.unit
    }
    pub fn input_fee_ppk(&self) -> u16 {
        self.input_fee_ppk
    }
}

impl From<memory_db::KeysetInfo<Unit>> for CachedKeysetInfo {
    fn from(value: memory_db::KeysetInfo<Unit>) -> Self {
        Self {
            active: value.active(),
            unit: value.unit(),
            input_fee_ppk: value.input_fee_ppk(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct KeysetCache {
    keysets: Arc<RwLock<HashMap<KeysetId, CachedKeysetInfo>>>,
}

impl KeysetCache {
    pub async fn get_keyset_info(
        &mut self,
        conn: &mut PgConnection,
        keyset_id: KeysetId,
    ) -> Result<CachedKeysetInfo, Error> {
        // happy path: the infos are already in the cache
        {
            let cache_read_lock = self.keysets.read();
            if let Some(info) = cache_read_lock.get(&keyset_id) {
                return Ok(info.clone());
            }
        }

        // Load the infos from db
        let keyset_info: CachedKeysetInfo = memory_db::get_keyset::<Unit>(conn, &keyset_id)
            .await?
            .into();

        // Save the infos in the cache
        {
            let mut cache_write_lock = self.keysets.write();
            cache_write_lock.insert(keyset_id, keyset_info.clone());
        }

        Ok(keyset_info)
    }
}

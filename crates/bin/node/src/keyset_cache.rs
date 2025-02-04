use std::{collections::HashMap, sync::Arc};

use memory_db::KeysetInfo;
use nuts::{
    nut01::{KeyPair, SetKeyPairs},
    nut02::KeysetId,
    Amount,
};
use parking_lot::RwLock;
use sqlx::PgConnection;

use keys_manager::KeysManager;

use crate::{errors::Error, Unit};

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
    keys: Arc<RwLock<HashMap<KeysetId, SetKeyPairs>>>,
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

    pub async fn get_key(
        &mut self,
        conn: &mut PgConnection,
        keys_manager: &KeysManager,
        keyset_id: KeysetId,
        amount: &Amount,
    ) -> Result<KeyPair, Error> {
        // Happy path: keys are already loaded in the cache
        {
            let keys_read_lock = self.keys.read();
            if let Some(keys) = keys_read_lock.get(&keyset_id) {
                let keypair = keys.get(amount).ok_or(Error::InvalidAmountKey)?;
                return Ok(keypair.clone());
            }
        }

        // Recover the keyset from the keyset infos stored in db
        let keyset_info: KeysetInfo<Unit> = memory_db::get_keyset(conn, &keyset_id).await?;
        let keyset = keys_manager.generate_keyset(
            keyset_info.unit(),
            keyset_info.derivation_path_index(),
            keyset_info.max_order(),
        );
        if keyset.id != keyset_id {
            return Err(Error::GeneratedKeysetIdIsDifferentFromOriginal);
        }

        // Clone the keypair we want to return
        let keypair = keyset
            .keys
            .get(amount)
            .ok_or(Error::InvalidAmountKey)?
            .clone();

        // Save the keys in the cache
        {
            let mut keys_write_lock = self.keys.write();
            keys_write_lock.insert(keyset_id, keyset.keys);
        }

        Ok(keypair)
    }
}

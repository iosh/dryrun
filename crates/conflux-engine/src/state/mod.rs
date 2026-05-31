mod rpc_state_key;

use std::{collections::HashMap, sync::Mutex};

use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::StateDb;
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use primitives::{EpochId, StorageKeyWithSpace};

use self::rpc_state_key::{RpcStateKey, RpcStateKeyError};

pub fn new_state_db(storage: Box<dyn StorageStateTrait>) -> StateDb {
    StateDb::new(storage)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionContext {
    // Cache keys must include block/epoch context.
    EspaceBlock { block_id: String },
    NativeEpoch { epoch: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    context: ExecutionContext,
    key_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
enum CachedRead {
    Present(Box<[u8]>),
    // Only use for states proven missing, not unsupported or failed reads.
    Absent,
}

#[derive(Debug, Default, Clone)]
pub struct StorageReadStats {
    pub get_calls: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub unsupported: u64,
}

#[derive(Debug)]
pub struct RpcBackedStorage {
    context: ExecutionContext,
    // Simulation writes shadow remote state.
    overlay: Mutex<HashMap<Vec<u8>, CachedRead>>,
    // Per-simulation remote read cache.
    cache: Mutex<HashMap<CacheKey, CachedRead>>,
    // Minimal counters for missing coverage and repeated reads.
    stats: Mutex<StorageReadStats>,
}

impl RpcBackedStorage {
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            context,
            overlay: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            stats: Mutex::new(StorageReadStats::default()),
        }
    }

    pub fn stats(&self) -> StorageResult<StorageReadStats> {
        Ok(self
            .stats
            .lock()
            .map_err(|_| Self::lock_error("stats"))?
            .clone())
    }

    fn unsupported(&self, operation: &'static str, key: StorageKeyWithSpace<'_>) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, context={:?}, key={:?}",
            self.context, key
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_storage_key(
        &self,
        operation: &'static str,
        key: StorageKeyWithSpace<'_>,
        error: RpcStateKeyError,
    ) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage key: operation={operation}, context={:?}, key={key:?}, reason={error}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unresolved_rpc_mapping(
        &self,
        operation: &'static str,
        rpc_key: &RpcStateKey,
        key: StorageKeyWithSpace<'_>,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage mapping not implemented yet: operation={operation}, context={:?}, rpc_key={rpc_key:?}, key={key:?}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_operation(&self, operation: &'static str) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, context={:?}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_commit(&self, epoch: EpochId) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation=commit, context={:?}, epoch={epoch:?}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn cache_key(&self, access_key: StorageKeyWithSpace<'_>) -> CacheKey {
        CacheKey {
            context: self.context.clone(),
            key_bytes: access_key.to_key_bytes(),
        }
    }

    fn cached_read_to_result(read: CachedRead) -> Option<Box<[u8]>> {
        match read {
            CachedRead::Present(value) => Some(value),
            CachedRead::Absent => None,
        }
    }

    fn record_stats(&self, update: impl FnOnce(&mut StorageReadStats)) -> StorageResult<()> {
        let mut stats = self.stats.lock().map_err(|_| Self::lock_error("stats"))?;
        update(&mut stats);
        Ok(())
    }

    fn lock_error(name: &'static str) -> StorageError {
        StorageError::Msg(format!("rpc-backed storage {name} mutex poisoned"))
    }
}

impl StorageStateTrait for RpcBackedStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
        self.record_stats(|stats| stats.get_calls += 1)?;

        let key_bytes = access_key.to_key_bytes();
        // Overlay writes shadow the remote snapshot for this simulation.
        if let Some(read) = self
            .overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .get(&key_bytes)
            .cloned()
        {
            return Ok(Self::cached_read_to_result(read));
        }

        let cache_key = self.cache_key(access_key);
        if let Some(read) = self
            .cache
            .lock()
            .map_err(|_| Self::lock_error("cache"))?
            .get(&cache_key)
            .cloned()
        {
            self.record_stats(|stats| stats.cache_hits += 1)?;
            return Ok(Self::cached_read_to_result(read));
        }

        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let rpc_key = match RpcStateKey::from_storage_key(access_key) {
            Ok(rpc_key) => rpc_key,
            Err(error) => {
                self.record_stats(|stats| {
                    stats.cache_misses += 1;
                    stats.unsupported += 1;
                })?;
                return Err(self.unsupported_storage_key("get", access_key, error));
            }
        };

        self.record_stats(|stats| {
            stats.cache_misses += 1;
            stats.unsupported += 1;
        })?;

        Err(self.unresolved_rpc_mapping("get", &rpc_key, access_key))
    }

    fn set(&mut self, access_key: StorageKeyWithSpace, value: Box<[u8]>) -> StorageResult<()> {
        self.overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .insert(access_key.to_key_bytes(), CachedRead::Present(value));
        Ok(())
    }

    fn delete(&mut self, access_key: StorageKeyWithSpace) -> StorageResult<()> {
        self.overlay
            .lock()
            .map_err(|_| Self::lock_error("overlay"))?
            .insert(access_key.to_key_bytes(), CachedRead::Absent);
        Ok(())
    }
    fn delete_test_only(
        &mut self,
        access_key: StorageKeyWithSpace,
    ) -> StorageResult<Option<Box<[u8]>>> {
        Err(self.unsupported("delete_test_only", access_key))
    }
    fn delete_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(self.unsupported("delete_all", access_key_prefix))
    }

    fn read_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(self.unsupported("read_all", access_key_prefix))
    }

    fn read_all_with_callback(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
        _callback: &mut dyn FnMut(MptKeyValue),
        _only_account_key: bool,
    ) -> StorageResult<()> {
        Err(self.unsupported("read_all_with_callback", access_key_prefix))
    }

    fn compute_state_root(&mut self) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_operation("compute_state_root"))
    }

    fn get_state_root(&self) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_operation("get_state_root"))
    }

    fn commit(&mut self, epoch: EpochId) -> StorageResult<StateRootWithAuxInfo> {
        Err(self.unsupported_commit(epoch))
    }
}

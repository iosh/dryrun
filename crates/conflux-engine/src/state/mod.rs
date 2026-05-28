use std::{collections::HashMap, sync::Mutex};

use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::StateDb;
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use primitives::{EpochId, StorageKey, StorageKeyWithSpace};

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
pub struct StorageReadStates {
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
    stats: Mutex<StorageReadStates>,
}

impl RpcBackedStorage {
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            context,
            overlay: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            stats: Mutex::new(StorageReadStates::default()),
        }
    }

    pub fn stats(&self) -> StorageReadStates {
        self.stats.lock().expect("stats mutex poisoned").clone()
    }

    fn unsupported(&self, operation: &'static str, key: StorageKeyWithSpace<'_>) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, context={:?}, key={:?}",
            self.context, key
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }
}

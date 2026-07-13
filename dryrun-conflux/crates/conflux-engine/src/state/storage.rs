use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cfx_executor::state::State;
use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::{Result as StateDbResult, StateDb};
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use primitives::{EpochId, StorageKeyWithSpace};
use tokio::runtime::Handle;

use crate::state::{
    ConfluxStatePoint,
    provider::RemoteStateProvider,
    reader::RemoteStateReader,
    state_item::{StateItem, StateItemError},
};

pub(crate) fn new_rpc_backed_state(
    state_point: ConfluxStatePoint,
    provider: Arc<dyn RemoteStateProvider>,
    runtime_handle: Handle,
) -> StateDbResult<State> {
    let storage = RpcBackedStorage::new(state_point, provider, runtime_handle);
    let db = StateDb::new(Box::new(storage));

    State::new(db)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    key_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
enum CachedRead {
    Present(Box<[u8]>),
    // Only use for states proven missing, not unsupported or failed reads.
    Absent,
}

pub(crate) struct RpcBackedStorage {
    state_point: ConfluxStatePoint,
    reader: RemoteStateReader,
    runtime_handle: Handle,
    // Simulation writes shadow remote state.
    overlay: Mutex<HashMap<Vec<u8>, CachedRead>>,
    // Per-simulation remote read cache.
    cache: Mutex<HashMap<CacheKey, CachedRead>>,
}

impl RpcBackedStorage {
    fn new(
        state_point: ConfluxStatePoint,
        provider: Arc<dyn RemoteStateProvider>,
        runtime_handle: Handle,
    ) -> Self {
        Self {
            state_point: state_point.clone(),
            reader: RemoteStateReader::new(state_point, provider),
            runtime_handle,
            overlay: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn unsupported(&self, operation: &'static str, key: StorageKeyWithSpace<'_>) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, state={:?}, key={:?}",
            self.state_point, key
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_storage_key(
        &self,
        operation: &'static str,
        key: StorageKeyWithSpace<'_>,
        error: StateItemError,
    ) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage key: operation={operation}, state={:?}, key={key:?}, reason={error}",
            self.state_point
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_operation(&self, operation: &'static str) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation={operation}, state={:?}",
            self.state_point
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn unsupported_commit(&self, epoch: EpochId) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage operation: operation=commit, state={:?}, epoch={epoch:?}",
            self.state_point
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn cache_key(&self, access_key: StorageKeyWithSpace<'_>) -> CacheKey {
        CacheKey {
            key_bytes: access_key.to_key_bytes(),
        }
    }

    fn cached_read_to_result(read: CachedRead) -> Option<Box<[u8]>> {
        match read {
            CachedRead::Present(value) => Some(value),
            CachedRead::Absent => None,
        }
    }

    fn lock_error(name: &'static str) -> StorageError {
        StorageError::Msg(format!("rpc-backed storage {name} mutex poisoned"))
    }
}

impl StorageStateTrait for RpcBackedStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
        let key_bytes = access_key.to_key_bytes();
        // Overlay writes shadow the remote state for this simulation.
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
            return Ok(Self::cached_read_to_result(read));
        }

        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let item = match StateItem::from_storage_key(access_key) {
            Ok(item) => item,
            Err(error) => return Err(self.unsupported_storage_key("get", access_key, error)),
        };

        let value = self.runtime_handle.block_on(self.reader.read(&item))?;

        let cached_read = match &value {
            Some(bytes) => CachedRead::Present(bytes.clone()),
            None => CachedRead::Absent,
        };

        self.cache
            .lock()
            .map_err(|_| Self::lock_error("cache"))?
            .insert(cache_key, cached_read);

        Ok(value)
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

mod codec;
mod provider;
mod request;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::StateDb;
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use cfx_types::{Address, H256};
use primitives::{EpochId, StorageKeyWithSpace};

use self::{
    codec::{StateValueCodecError, encode_espace_code, encode_espace_storage_slot},
    provider::{RemoteStateProvider, RemoteStateProviderError},
    request::{StateReadRequest, StateReadRequestError},
};

pub fn new_state_db(storage: Box<dyn StorageStateTrait>) -> StateDb {
    StateDb::new(storage)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecutionContext {
    EspaceBlock { block_id: String },
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

pub struct RpcBackedStorage {
    context: ExecutionContext,
    provider: Arc<dyn RemoteStateProvider>,
    // Simulation writes shadow remote state.
    overlay: Mutex<HashMap<Vec<u8>, CachedRead>>,
    // Per-simulation remote read cache.
    cache: Mutex<HashMap<CacheKey, CachedRead>>,
}

impl RpcBackedStorage {
    pub fn new(context: ExecutionContext, provider: Arc<dyn RemoteStateProvider>) -> Self {
        Self {
            context,
            provider,
            overlay: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
        }
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
        error: StateReadRequestError,
    ) -> StorageError {
        let message = format!(
            "unsupported rpc-backed storage key: operation={operation}, context={:?}, key={key:?}, reason={error}",
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

    // This is the future handoff point from semantic RPC state into Conflux raw bytes.
    fn fetch_rpc_value(&self, rpc_key: &StateReadRequest) -> StorageResult<Option<Box<[u8]>>> {
        match rpc_key {
            StateReadRequest::EspaceStorageSlot { address, slot } => {
                self.fetch_espace_storage_slot(*address, *slot)
            }
            StateReadRequest::EspaceCode { address, code_hash } => {
                self.fetch_espace_code(*address, *code_hash)
            }
        }
    }

    fn fetch_espace_storage_slot(
        &self,
        address: Address,
        slot: H256,
    ) -> StorageResult<Option<Box<[u8]>>> {
        let block_id = match &self.context {
            ExecutionContext::EspaceBlock { block_id } => block_id.as_str(),
        };

        let value = self
            .provider
            .get_espace_storage_at(block_id, address, slot)
            .map_err(|error| self.provider_error("get_espace_storage_at", error))?;

        Ok(value.map(encode_espace_storage_slot))
    }

    fn fetch_espace_code(
        &self,
        address: Address,
        expected_code_hash: H256,
    ) -> StorageResult<Option<Box<[u8]>>> {
        let block_id = match &self.context {
            ExecutionContext::EspaceBlock { block_id } => block_id.as_str(),
        };

        let code = self
            .provider
            .get_espace_code_at(block_id, address)
            .map_err(|error| self.provider_error("get_espace_code_at", error))?;

        if code.is_empty() {
            return Ok(None);
        }

        encode_espace_code(expected_code_hash, code)
            .map(Some)
            .map_err(|error| self.codec_error("encode_espace_code", error))
    }

    fn provider_error(
        &self,
        operation: &'static str,
        error: RemoteStateProviderError,
    ) -> StorageError {
        let message = format!(
            "rpc-backed storage provider error: operation={operation}, context={:?},
              reason={error}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn codec_error(&self, operation: &'static str, error: StateValueCodecError) -> StorageError {
        let message = format!(
            "rpc-backed storage codec error: operation={operation}, context={:?}, reason={error}",
            self.context
        );
        tracing::warn!("{message}");
        StorageError::Msg(message)
    }

    fn lock_error(name: &'static str) -> StorageError {
        StorageError::Msg(format!("rpc-backed storage {name} mutex poisoned"))
    }
}

impl StorageStateTrait for RpcBackedStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
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
            return Ok(Self::cached_read_to_result(read));
        }

        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let rpc_key = match StateReadRequest::from_storage_key(access_key) {
            Ok(rpc_key) => rpc_key,
            Err(error) => return Err(self.unsupported_storage_key("get", access_key, error)),
        };

        let value = self.fetch_rpc_value(&rpc_key)?;

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

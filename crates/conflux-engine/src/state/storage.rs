use cfx_executor::state::State;
use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::{Result as StateDbResult, StateDb};
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use primitives::{EpochId, StorageKeyWithSpace};
use tokio::runtime::Handle;

use crate::state::{
    ConfluxStatePoint,
    reader::RemoteStateReader,
    state_item::{StateItem, StateItemError},
};

pub(crate) fn new_rpc_backed_state(
    reader: RemoteStateReader,
    runtime_handle: Handle,
) -> StateDbResult<State> {
    let storage = RpcBackedStorage::new(reader, runtime_handle);
    let db = StateDb::new(Box::new(storage));

    State::new(db)
}

pub(crate) struct RpcBackedStorage {
    state_point: ConfluxStatePoint,
    reader: RemoteStateReader,
    runtime_handle: Handle,
}

impl RpcBackedStorage {
    fn new(reader: RemoteStateReader, runtime_handle: Handle) -> Self {
        Self {
            state_point: reader.state_point().clone(),
            reader,
            runtime_handle,
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
}

impl StorageStateTrait for RpcBackedStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let item = match StateItem::from_storage_key(access_key) {
            Ok(item) => item,
            Err(error) => return Err(self.unsupported_storage_key("get", access_key, error)),
        };

        self.runtime_handle.block_on(self.reader.read(&item))
    }

    fn set(&mut self, access_key: StorageKeyWithSpace, _value: Box<[u8]>) -> StorageResult<()> {
        Err(self.unsupported("set", access_key))
    }

    fn delete(&mut self, access_key: StorageKeyWithSpace) -> StorageResult<()> {
        Err(self.unsupported("delete", access_key))
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

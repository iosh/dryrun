use super::{StateSource, state_item::StateItem};
use crate::StateError;
use cfx_executor::state::State;
use cfx_internal_common::StateRootWithAuxInfo;
use cfx_statedb::{Result as StateDbResult, StateDb};
use cfx_storage::{Error as StorageError, MptKeyValue, Result as StorageResult, StorageStateTrait};
use primitives::{EpochId, StorageKeyWithSpace};
use std::sync::Arc;
use tokio::runtime::Handle;

pub(crate) fn new_state(source: Arc<StateSource>, runtime_handle: Handle) -> StateDbResult<State> {
    State::new(StateDb::new(Box::new(RpcStorage {
        source,
        runtime_handle,
    })))
}

struct RpcStorage {
    source: Arc<StateSource>,
    runtime_handle: Handle,
}

fn unsupported(operation: &str, key: StorageKeyWithSpace<'_>) -> StorageError {
    StateError::Unavailable(format!("RPC storage cannot {operation} {key:?}")).into()
}
fn unsupported_operation(operation: &str) -> StorageError {
    StateError::Unavailable(format!("RPC storage cannot {operation}")).into()
}

impl StorageStateTrait for RpcStorage {
    fn get(&self, access_key: StorageKeyWithSpace) -> StorageResult<Option<Box<[u8]>>> {
        // Before we can fetch anything from RPC, we need to understand which
        // semantic state item this raw storage key refers to.
        let item = StateItem::from_storage_key(access_key)
            .map_err(|error| StorageError::from(StateError::Unavailable(error.to_string())))?;

        self.runtime_handle
            .block_on(self.source.read(item))
            .map_err(Into::into)
    }

    fn set(&mut self, access_key: StorageKeyWithSpace, _value: Box<[u8]>) -> StorageResult<()> {
        Err(unsupported("set", access_key))
    }

    fn delete(&mut self, access_key: StorageKeyWithSpace) -> StorageResult<()> {
        Err(unsupported("delete", access_key))
    }

    fn delete_test_only(
        &mut self,
        access_key: StorageKeyWithSpace,
    ) -> StorageResult<Option<Box<[u8]>>> {
        Err(unsupported("delete_test_only", access_key))
    }

    fn delete_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(unsupported("delete_all", access_key_prefix))
    }

    fn read_all(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
    ) -> StorageResult<Option<Vec<MptKeyValue>>> {
        Err(unsupported("read_all", access_key_prefix))
    }

    fn read_all_with_callback(
        &mut self,
        access_key_prefix: StorageKeyWithSpace,
        _callback: &mut dyn FnMut(MptKeyValue),
        _only_account_key: bool,
    ) -> StorageResult<()> {
        Err(unsupported("read_all_with_callback", access_key_prefix))
    }

    fn compute_state_root(&mut self) -> StorageResult<StateRootWithAuxInfo> {
        Err(unsupported_operation("compute_state_root"))
    }

    fn get_state_root(&self) -> StorageResult<StateRootWithAuxInfo> {
        Err(unsupported_operation("get_state_root"))
    }

    fn commit(&mut self, _epoch: EpochId) -> StorageResult<StateRootWithAuxInfo> {
        Err(unsupported_operation("commit"))
    }
}

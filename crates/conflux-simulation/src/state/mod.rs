mod core_space_internal;
mod provider;
mod reader;
mod rpc_types;
mod state_item;
mod state_value_encoding;
mod storage;

use alloy::{eips::BlockId as EspaceBlockId, primitives::B256};
use cfx_types::H256;
use conflux_provider::{BlockHashOrEpochNumber, EpochNumber};

pub use self::provider::ConfluxRpcError;

pub(crate) use self::{
    core_space_internal::SponsorWhitelistStorageKey,
    provider::ConfluxSimulationProvider,
    reader::ConfluxStateSource,
    rpc_types::{CoreSpaceRpcBlock, EspaceRpcBlock},
    storage::new_conflux_state,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ConfluxStateAnchor {
    epoch_number: u64,
    pivot_hash: H256,
}

impl ConfluxStateAnchor {
    pub(crate) fn new(epoch_number: u64, pivot_hash: H256) -> Self {
        Self {
            epoch_number,
            pivot_hash,
        }
    }

    pub(crate) fn epoch_number(&self) -> u64 {
        self.epoch_number
    }

    pub(crate) fn pivot_hash(&self) -> H256 {
        self.pivot_hash
    }

    pub(crate) fn espace_block(&self) -> EspaceBlockId {
        EspaceBlockId::hash_canonical(B256::from_slice(self.pivot_hash.as_bytes()))
    }

    pub(crate) fn core_space_pivot(&self) -> BlockHashOrEpochNumber {
        BlockHashOrEpochNumber::BlockHash {
            hash: B256::from_slice(self.pivot_hash.as_bytes()),
            require_pivot: Some(true),
        }
    }

    pub(crate) fn core_space_epoch(&self) -> EpochNumber {
        EpochNumber::Number(self.epoch_number)
    }
}

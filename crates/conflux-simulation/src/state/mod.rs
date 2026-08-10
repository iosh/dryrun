mod core_space_internal;
mod phases;
mod provider;
mod reader;
mod rpc_types;
mod state_item;
mod state_value_encoding;
mod storage;

use alloy::{eips::BlockId as EspaceBlockId, primitives::B256};
use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_types::{H256, U64};
use conflux_provider::BlockHashOrEpochNumber;

pub use self::provider::ConfluxRpcError;

pub(crate) use self::{
    core_space_internal::SponsorWhitelistStorageKey,
    phases::{StatePhaseValues, execute_with_state_phases},
    provider::{ConfluxSimulationProvider, EspaceEstimateTransaction},
    reader::{AnchoredVoteLists, ConfluxStateSource, MaskedSponsorWhitelistEntries},
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

    pub(crate) fn core_space_epoch(&self) -> CfxEpochNumber {
        CfxEpochNumber::Num(U64::from(self.epoch_number))
    }
}

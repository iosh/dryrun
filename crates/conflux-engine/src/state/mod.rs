mod core_space_internal;
mod http_provider;
mod reader;
mod rpc_types;
mod state_item;
mod state_value_encoding;
mod storage;

use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_types::{H256, U64};

pub use self::http_provider::{ConfluxRpcError, CoreSpaceResourceEstimate, HttpConfluxProvider};

pub(crate) use self::{
    reader::RemoteStateReader,
    rpc_types::{CoreSpaceRpcBlock, EspaceRpcBlock},
    storage::new_rpc_backed_state,
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

    pub(crate) fn espace_block(&self) -> EthBlockId {
        EthBlockId::Num(self.epoch_number)
    }

    pub(crate) fn core_space_epoch(&self) -> CfxEpochNumber {
        CfxEpochNumber::Num(U64::from(self.epoch_number))
    }
}

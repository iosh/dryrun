mod http_provider;
mod native_internal;
mod provider;
mod reader;
mod rpc_encoding;
mod rpc_types;
mod state_item;
mod state_value_encoding;
mod storage;

use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_types::{H256, U64};

pub use self::{
    http_provider::HttpConfluxStateProvider,
    provider::{RemoteStateProvider, RemoteStateProviderError},
    rpc_types::{
        EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock,
        NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
    },
};

pub(crate) use self::storage::new_rpc_backed_state;

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

    pub(crate) fn espace_block(&self) -> EthBlockId {
        EthBlockId::Num(self.epoch_number)
    }

    pub(crate) fn native_epoch(&self) -> CfxEpochNumber {
        CfxEpochNumber::Num(U64::from(self.epoch_number))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfluxStatePoint {
    anchor: ConfluxStateAnchor,
}

impl ConfluxStatePoint {
    pub(crate) fn from_anchor(anchor: ConfluxStateAnchor) -> Self {
        Self { anchor }
    }

    pub(crate) fn anchor(&self) -> &ConfluxStateAnchor {
        &self.anchor
    }

    pub(crate) fn espace_block(&self) -> EthBlockId {
        self.anchor.espace_block()
    }

    pub(crate) fn native_epoch(&self) -> CfxEpochNumber {
        self.anchor.native_epoch()
    }
}

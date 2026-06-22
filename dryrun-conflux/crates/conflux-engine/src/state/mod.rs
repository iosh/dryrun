mod http_provider;
mod provider;
mod reader;
mod rpc_encoding;
mod rpc_types;
mod state_item;
mod state_value_encoding;
mod storage;

use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;

pub use self::{
    http_provider::HttpConfluxStateProvider,
    provider::{RemoteStateProvider, RemoteStateProviderError},
    rpc_types::{
        EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock,
        NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
    },
};

pub(crate) use self::storage::new_rpc_backed_state;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfluxStatePoint {
    pub espace_block: EthBlockId,
    pub native_epoch: CfxEpochNumber,
}

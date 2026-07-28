use async_trait::async_trait;
use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::BlockId;
use jsonrpsee::rpc_params;

use crate::state::{
    provider::{ConfluxBlockProvider, RemoteStateProviderError},
    rpc_types::{CoreSpaceRpcBlock, EspaceRpcBlock},
};

use super::HttpConfluxProvider;

#[async_trait]
impl ConfluxBlockProvider for HttpConfluxProvider {
    async fn cfx_get_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, RemoteStateProviderError> {
        self.core_space_rpc_request(
            "cfx_getBlockByEpochNumber",
            rpc_params![epoch_number, false],
        )
        .await
    }

    async fn eth_get_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBlockByNumber", rpc_params![block_number, false])
            .await
    }
}

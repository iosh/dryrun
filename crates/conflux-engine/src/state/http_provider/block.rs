use crate::state::{
    ConfluxRpcError,
    rpc_types::{CoreSpaceRpcBlock, CoreSpaceRpcPoSBlock, EspaceRpcBlock},
};
use alloy_primitives::B256;
use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::BlockId;
use cfx_types::H256;
use jsonrpsee::rpc_params;

use super::HttpConfluxProvider;

impl HttpConfluxProvider {
    pub(crate) async fn cfx_get_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, ConfluxRpcError> {
        let block = Self::core_request(
            "cfx_getBlockByEpochNumber",
            self.core_space_provider
                .cfx_get_block_by_epoch_number(Self::provider_epoch(epoch_number)?, false),
        )
        .await?;
        block
            .map(|block| self.convert_core_block(block))
            .transpose()
    }

    pub(crate) async fn eth_get_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, ConfluxRpcError> {
        self.espace_rpc_request("eth_getBlockByNumber", rpc_params![block_number, false])
            .await
    }

    pub(crate) async fn pos_get_block_by_hash(
        &self,
        block_hash: H256,
    ) -> Result<Option<CoreSpaceRpcPoSBlock>, ConfluxRpcError> {
        let block = Self::core_request(
            "pos_getBlockByHash",
            self.core_space_provider
                .pos_get_block_by_hash(B256::from_slice(block_hash.as_bytes())),
        )
        .await?;
        block
            .map(|block| {
                Ok(CoreSpaceRpcPoSBlock {
                    hash: cfx_types::H256::from_slice(block.hash.as_slice()),
                    height: cfx_types::U64::from(Self::alloy_u256_to_u64(
                        block.height,
                        "pos_getBlockByHash",
                        "height",
                    )?),
                    pivot_decision: block
                        .pivot_decision
                        .map(|decision| {
                            Ok(crate::state::rpc_types::CoreSpaceRpcPoSPivotDecision {
                                height: cfx_types::U64::from(Self::alloy_u256_to_u64(
                                    decision.height,
                                    "pos_getBlockByHash",
                                    "pivotDecision.height",
                                )?),
                            })
                        })
                        .transpose()?,
                })
            })
            .transpose()
    }

    fn convert_core_block(
        &self,
        block: conflux_provider::CoreRpcBlock,
    ) -> Result<CoreSpaceRpcBlock, ConfluxRpcError> {
        Ok(CoreSpaceRpcBlock {
            hash: cfx_types::H256::from_slice(block.hash.as_slice()),
            height: crate::primitive::u256_to_cfx(block.height),
            miner: Self::provider_address_to_rpc(block.miner)?,
            block_number: block.block_number.map(crate::primitive::u256_to_cfx),
            base_fee_per_gas: block.base_fee_per_gas.map(crate::primitive::u256_to_cfx),
            timestamp: crate::primitive::u256_to_cfx(block.timestamp),
            pos_reference: block
                .pos_reference
                .map(|hash| cfx_types::H256::from_slice(hash.as_slice())),
        })
    }
}

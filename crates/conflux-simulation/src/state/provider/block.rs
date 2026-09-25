use crate::state::{
    ConfluxRpcError, ConfluxStateAnchor,
    rpc_types::{CoreSpaceRpcBlock, CoreSpaceRpcPoSBlock, EspaceRpcBlock},
};
use alloy::{consensus::BlockHeader, eips::BlockId, primitives::B256, providers::Provider};
use cfx_types::H256;
use conflux_provider::EpochNumber;

use super::ConfluxSimulationProvider;

impl ConfluxSimulationProvider {
    /// Checks the fixed height on both endpoints. This detects observed pivot
    /// changes; separate RPC requests do not form an atomic state snapshot.
    pub(crate) async fn validate_state_anchor(
        &self,
        anchor: ConfluxStateAnchor,
    ) -> Result<(), crate::ConfluxStateAnchorError> {
        let epoch_number = anchor.epoch_number();
        let (core_block, espace_block) = tokio::try_join!(
            self.cfx_get_block_by_epoch_number(EpochNumber::Number(epoch_number)),
            self.eth_get_block(BlockId::number(epoch_number)),
        )?;
        let expected_pivot_hash = crate::primitive::b256_from_cfx(anchor.pivot_hash());
        let core_space_pivot_hash =
            core_block.map(|block| crate::primitive::b256_from_cfx(block.hash));
        let espace_block_hash = espace_block.map(|block| block.hash);
        if core_space_pivot_hash != Some(expected_pivot_hash)
            || espace_block_hash != Some(expected_pivot_hash)
        {
            return Err(crate::ConfluxStateAnchorError::Mismatch {
                epoch_number,
                expected_pivot_hash,
                core_space_pivot_hash,
                espace_block_hash,
            });
        }
        Ok(())
    }

    pub(crate) async fn cfx_get_block_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<CoreSpaceRpcBlock>, ConfluxRpcError> {
        let block = Self::core_request(
            "cfx_getBlockByHash",
            self.core_space_provider.cfx_get_block_by_hash(hash, false),
        )
        .await?;
        Ok(block.map(Self::convert_core_block))
    }

    pub(crate) async fn cfx_get_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, ConfluxRpcError> {
        let block = Self::core_request(
            "cfx_getBlockByEpochNumber",
            self.core_space_provider
                .cfx_get_block_by_epoch_number(epoch_number, false),
        )
        .await?;
        Ok(block.map(Self::convert_core_block))
    }

    pub(crate) async fn eth_get_block(
        &self,
        block: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, ConfluxRpcError> {
        let operation = if block.is_hash() {
            "eth_getBlockByHash"
        } else {
            "eth_getBlockByNumber"
        };
        let response = self
            .espace_provider
            .get_block(block)
            .await
            .map_err(|error| ConfluxRpcError::Espace {
                operation,
                source: error,
            })?;

        Ok(response.map(|block| {
            let hash = block.hash();
            let header = block.into_consensus_header();
            EspaceRpcBlock {
                hash,
                number: header.number(),
                base_fee_per_gas: header.base_fee_per_gas().map(cfx_types::U256::from),
            }
        }))
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

    fn convert_core_block(block: conflux_provider::CoreRpcBlock) -> CoreSpaceRpcBlock {
        CoreSpaceRpcBlock {
            hash: cfx_types::H256::from_slice(block.hash.as_slice()),
            epoch_number: block.epoch_number.map(crate::primitive::u256_to_cfx),
            miner: cfx_types::Address::from(block.miner.bytes()),
            block_number: block.block_number.map(crate::primitive::u256_to_cfx),
            base_fee_per_gas: block.base_fee_per_gas.map(crate::primitive::u256_to_cfx),
            timestamp: crate::primitive::u256_to_cfx(block.timestamp),
            pos_reference: block
                .pos_reference
                .map(|hash| cfx_types::H256::from_slice(hash.as_slice())),
        }
    }
}

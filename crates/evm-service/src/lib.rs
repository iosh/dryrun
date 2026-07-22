mod error;

use std::sync::Arc;

use alloy::{
    consensus::{Header, Sealable},
    eips::{BlockId, BlockNumberOrTag},
    primitives::B256,
    providers::{DynProvider, Provider},
};
use evm_engine::{EvmEngine, EvmExecutionInput, ResolvedBlock};
use simulation_tasks::SimulationTaskSet;

pub use error::SimulationServiceError;
pub use evm_engine::{
    AccessListItem, Change, Erc20Metadata, Erc721CollectionMetadata,
    EvmExecution as SimulationExecution, EvmExecutionFailure as ExecutionFailure,
    EvmExecutionFailureCode, EvmExecutionOutcome as ExecutionOutcome,
    EvmSimulation as SimulateEvmTransactionOutput, EvmTransaction, EvmTransactionVariant,
    NativeMetadata, SimulatedBlock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockSelector {
    Latest,
    Safe,
    Finalized,
    Number(u64),
}

impl BlockSelector {
    fn block_id(self) -> BlockId {
        let block_number_or_tag = match self {
            Self::Latest => BlockNumberOrTag::Latest,
            Self::Safe => BlockNumberOrTag::Safe,
            Self::Finalized => BlockNumberOrTag::Finalized,
            Self::Number(number) => BlockNumberOrTag::Number(number),
        };

        BlockId::Number(block_number_or_tag)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionInput {
    pub block: BlockSelector,
    pub transaction: EvmTransaction,
}

#[derive(Debug, Clone)]
pub struct SimulationService {
    provider: DynProvider,
    evm_engine: Arc<EvmEngine>,
    simulation_tasks: SimulationTaskSet,
}

impl SimulationService {
    pub fn new(
        provider: DynProvider,
        evm_engine: Arc<EvmEngine>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            provider,
            evm_engine,
            simulation_tasks,
        }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        let SimulateEvmTransactionInput { block, transaction } = input;
        let provider = self.provider.clone();
        let evm_engine = Arc::clone(&self.evm_engine);

        self.simulation_tasks
            .run(move || async move {
                let resolved_block = resolve_block(&provider, block).await?;

                tokio::task::spawn_blocking(move || {
                    evm_engine.simulate(EvmExecutionInput {
                        block: resolved_block,
                        transaction,
                    })
                })
                .await
                .map_err(SimulationServiceError::execution_task)?
                .map_err(Into::into)
            })
            .await
            .map_err(SimulationServiceError::from)?
    }
}

async fn resolve_block(
    provider: &DynProvider,
    selector: BlockSelector,
) -> Result<ResolvedBlock, SimulationServiceError> {
    let block = provider
        .get_block(selector.block_id())
        .await
        .map_err(|_| {
            SimulationServiceError::block_resolution(
                "provider request failed while resolving block",
            )
        })?
        .ok_or_else(|| {
            SimulationServiceError::block_resolution("provider did not return the requested block")
        })?;

    let provider_hash = block.hash();
    let header = block.into_consensus_header();

    resolved_block_from_header(header, provider_hash)
}

fn resolved_block_from_header(
    header: Header,
    provider_hash: B256,
) -> Result<ResolvedBlock, SimulationServiceError> {
    let sealed_header = header.seal_slow();

    if sealed_header.hash() != provider_hash {
        return Err(SimulationServiceError::block_resolution(
            "provider block hash did not match the recomputed header hash",
        ));
    }

    Ok(ResolvedBlock::new(sealed_header))
}

#[cfg(test)]
mod tests {
    use alloy::{
        consensus::{Header, Sealable},
        primitives::B256,
    };

    use super::resolved_block_from_header;

    #[test]
    fn resolved_block_requires_a_matching_recomputed_header_hash() {
        let header = Header::default();
        let expected_hash = header.clone().seal_slow().hash();

        let resolved = resolved_block_from_header(header.clone(), expected_hash)
            .expect("matching block hash should resolve");
        assert_eq!(resolved.hash(), expected_hash);

        let error = resolved_block_from_header(header, B256::ZERO)
            .expect_err("mismatched block hash should be rejected");
        assert_eq!(error.kind_code(), Some("block_resolution_error"));
    }
}

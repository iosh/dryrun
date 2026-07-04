pub mod config;
mod error;
pub mod execution;
mod simulation;
pub mod state;
mod transaction;

use std::sync::Arc;

use crate::{
    config::ConfluxConfig,
    execution::{
        ExecutionBlockContext, ExecutionConsensusContext, TransactionExecutionInput,
        build_espace_block_context, build_execution_block_context, build_mainnet_machine,
        build_native_pivot_block_context, build_rpc_backed_state, execute_transaction,
    },
    simulation::build_espace_execution,
    state::{
        ConfluxStatePoint, EspaceRpcBlock, HttpConfluxStateProvider, NativeRpcBlock,
        RemoteStateProvider,
    },
    transaction::build_espace_transaction_input,
};
use cfx_types::{U64, U256};
pub use error::ConfluxEngineError;
pub use simulation::{
    EspaceExecution, EspaceExecutionFailure, EspaceExecutionStatus, EspaceSimulation,
    SimulatedBlock,
};
pub use transaction::{
    AccessListItem, EspaceBlockRef, EspaceTransaction, EspaceTransactionType,
    SimulateEspaceTransactionInput,
};

use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;

pub struct ConfluxEngine {
    config: ConfluxConfig,
    provider: Arc<dyn RemoteStateProvider>,
}

impl ConfluxEngine {
    pub fn new(config: ConfluxConfig) -> Result<Self, ConfluxEngineError> {
        let provider = Arc::new(HttpConfluxStateProvider::new(config.clone())?);
        Ok(Self { config, provider })
    }

    pub fn with_provider(config: ConfluxConfig, provider: Arc<dyn RemoteStateProvider>) -> Self {
        Self { config, provider }
    }

    pub fn simulate_espace_transaction(
        &self,
        input: SimulateEspaceTransactionInput,
    ) -> Result<EspaceSimulation, ConfluxEngineError> {
        let SimulateEspaceTransactionInput { block, transaction } = input;
        let gas_limit = transaction.gas_limit;
        let transaction =
            build_espace_transaction_input(transaction, self.config.chain.evm_chain_id)?;
        let execution_context = self.resolve_espace_execution_context(&block)?;

        let execution_input = TransactionExecutionInput {
            block_context: execution_context.block_context,
            transaction,
        };

        let mut state =
            build_rpc_backed_state(execution_context.state_point, Arc::clone(&self.provider))
                .map_err(|error| ConfluxEngineError::StateAccess {
                    message: error.to_string(),
                })?;

        let machine = build_mainnet_machine();

        let outcome =
            execute_transaction(&mut state, &machine, execution_input).map_err(|error| {
                ConfluxEngineError::ExecutionInternal {
                    message: error.to_string(),
                }
            })?;

        Ok(EspaceSimulation::new(build_espace_execution(
            self.config.chain.evm_chain_id,
            execution_context.simulated_block,
            gas_limit,
            outcome,
        )))
    }

    fn resolve_espace_execution_context(
        &self,
        block: &EspaceBlockRef,
    ) -> Result<EspaceExecutionContext, ConfluxEngineError> {
        let selector = ResolvedBlockSelector::from_block_ref(block);
        let state_point = selector.state_point();
        let blocks = self.resolve_execution_blocks(selector)?;

        let simulated_block = SimulatedBlock {
            number: espace_block_number(&blocks.espace_block)?,
            hash: blocks.espace_block.hash,
        };

        let native_pivot = build_native_pivot_block_context(&blocks.native_pivot_block)?;
        let espace = build_espace_block_context(&blocks.espace_block);

        let block_context = build_execution_block_context(
            &native_pivot,
            &espace,
            ExecutionConsensusContext::default(),
        );

        Ok(EspaceExecutionContext {
            block_context,
            state_point,
            simulated_block,
        })
    }

    fn resolve_execution_blocks(
        &self,
        selector: ResolvedBlockSelector,
    ) -> Result<ExecutionBlocks, ConfluxEngineError> {
        let espace_block = self
            .provider
            .get_espace_block_by_number(selector.espace_block)?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "eSpace block".to_string(),
            })?;

        let native_pivot_block = self
            .provider
            .get_native_block_by_epoch_number(selector.native_epoch)?
            .ok_or_else(|| ConfluxEngineError::BlockNotFound {
                block: "native pivot block".to_string(),
            })?;

        Ok(ExecutionBlocks {
            espace_block,
            native_pivot_block,
        })
    }
}
struct EspaceExecutionContext {
    block_context: ExecutionBlockContext,
    state_point: ConfluxStatePoint,
    simulated_block: SimulatedBlock,
}

struct ExecutionBlocks {
    espace_block: EspaceRpcBlock,
    native_pivot_block: NativeRpcBlock,
}
struct ResolvedBlockSelector {
    espace_block: EthBlockId,
    native_epoch: CfxEpochNumber,
}

impl ResolvedBlockSelector {
    fn from_block_ref(block: &EspaceBlockRef) -> Self {
        match block {
            EspaceBlockRef::Latest => Self {
                espace_block: EthBlockId::Latest,
                native_epoch: CfxEpochNumber::LatestState,
            },
            EspaceBlockRef::Number(number) => Self {
                espace_block: EthBlockId::Num(*number),
                native_epoch: CfxEpochNumber::Num(U64::from(*number)),
            },
        }
    }

    fn state_point(&self) -> ConfluxStatePoint {
        ConfluxStatePoint {
            espace_block: self.espace_block,
            native_epoch: self.native_epoch.clone(),
        }
    }
}

fn espace_block_number(block: &EspaceRpcBlock) -> Result<u64, ConfluxEngineError> {
    if block.number > U256::from(u64::MAX) {
        return Err(ConfluxEngineError::InvalidBlockContext {
            message: format!("eSpace block number exceeds u64: {:?}", block.number),
        });
    }

    Ok(block.number.as_u64())
}

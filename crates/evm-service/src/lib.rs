mod error;

use std::sync::Arc;

use evm_engine::EvmEngine;

pub use error::SimulationServiceError;
pub use evm_engine::{
    AccessListItem, BlockRef, Change, Erc20Metadata, Erc721CollectionMetadata,
    EvmExecution as SimulationExecution, EvmExecutionFailure as ExecutionFailure,
    EvmExecutionFailureCode, EvmExecutionInput as SimulateEvmTransactionInput,
    EvmExecutionStatus as ExecutionStatus, EvmSimulation as SimulateEvmTransactionOutput,
    EvmTransaction, EvmTransactionVariant, NativeMetadata, SimulatedBlock,
};

#[derive(Debug, Clone)]
pub struct SimulationService {
    evm_engine: Arc<EvmEngine>,
}

impl SimulationService {
    pub fn new(evm_engine: Arc<EvmEngine>) -> Self {
        Self { evm_engine }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        self.evm_engine.simulate(input).await.map_err(Into::into)
    }
}

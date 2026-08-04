mod error;

use std::sync::Arc;

use evm_engine::{EvmEngine, EvmExecutionInput, ResolvedBlock};
use evm_simulation::EvmSimulationPreparer;
use simulation_tasks::SimulationTaskSet;

pub use error::SimulationServiceError;
pub use evm_engine::{
    AccessListItem, Change, Erc20Metadata, Erc721CollectionMetadata,
    EvmExecution as SimulationExecution, EvmExecutionFailure as ExecutionFailure,
    EvmExecutionFailureCode, EvmExecutionOutcome as ExecutionOutcome,
    EvmSimulation as SimulateEvmTransactionOutput, ExecutedDetails, NativeMetadata, SimulatedBlock,
};
pub use evm_simulation::EvmBlockSelector;
pub use simulation_transaction::TransactionRequest as EvmTransactionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionInput {
    pub block: EvmBlockSelector,
    pub transaction: EvmTransactionRequest,
}

#[derive(Debug, Clone)]
pub struct SimulationService {
    preparer: Arc<EvmSimulationPreparer>,
    evm_engine: Arc<EvmEngine>,
    simulation_tasks: SimulationTaskSet,
}

impl SimulationService {
    pub fn new(
        preparer: Arc<EvmSimulationPreparer>,
        evm_engine: Arc<EvmEngine>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            preparer,
            evm_engine,
            simulation_tasks,
        }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        let SimulateEvmTransactionInput { block, transaction } = input;
        let preparer = Arc::clone(&self.preparer);
        let evm_engine = Arc::clone(&self.evm_engine);

        self.simulation_tasks
            .run(move || async move {
                let prepared = preparer.prepare_transaction(block, transaction).await?;
                let (block, transaction) = prepared.into_parts();

                let simulation = tokio::task::spawn_blocking(move || {
                    evm_engine.simulate(EvmExecutionInput {
                        block: ResolvedBlock::new(block),
                        transaction,
                    })
                })
                .await
                .map_err(SimulationServiceError::execution_task)??;

                Ok(simulation)
            })
            .await?
    }
}

mod error;

use std::sync::Arc;

use evm_simulation::{EvmSimulationPreparer, EvmSimulator};
use simulation_tasks::SimulationTaskSet;

pub use error::EvmServiceError;
pub use evm_simulation::EvmBlockSelector;
pub use evm_simulation::{
    AccessListItem, Change, Erc20Metadata, Erc721CollectionMetadata, EvmBlockContext, EvmExecution,
    EvmExecutionDetails, EvmExecutionFailure, EvmExecutionFailureCode, EvmOutcome, EvmSimulation,
    NativeMetadata,
};
pub use simulation_transaction::TransactionRequest as EvmTransactionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulationInput {
    pub block: EvmBlockSelector,
    pub transaction: EvmTransactionRequest,
}

#[derive(Debug, Clone)]
pub struct EvmSimulationService {
    preparer: Arc<EvmSimulationPreparer>,
    simulator: Arc<EvmSimulator>,
    simulation_tasks: SimulationTaskSet,
}

impl EvmSimulationService {
    pub fn new(
        preparer: Arc<EvmSimulationPreparer>,
        simulator: Arc<EvmSimulator>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            preparer,
            simulator,
            simulation_tasks,
        }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: EvmSimulationInput,
    ) -> Result<EvmSimulation, EvmServiceError> {
        let EvmSimulationInput { block, transaction } = input;
        let preparer = Arc::clone(&self.preparer);
        let simulator = Arc::clone(&self.simulator);

        self.simulation_tasks
            .run(move || async move {
                let prepared = preparer.prepare_transaction(block, transaction).await?;

                let simulation = tokio::task::spawn_blocking(move || simulator.simulate(prepared))
                    .await
                    .map_err(EvmServiceError::execution_task)??;

                Ok(simulation)
            })
            .await?
    }
}

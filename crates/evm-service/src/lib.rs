mod error;

use std::sync::Arc;

use evm_simulation::{EvmSimulation, EvmSimulationRequest, EvmTransactionSimulator};
use simulation_tasks::SimulationTaskSet;

pub use error::EvmServiceError;

#[derive(Debug, Clone)]
pub struct EvmSimulationService {
    simulator: Arc<EvmTransactionSimulator>,
    simulation_tasks: SimulationTaskSet,
}

impl EvmSimulationService {
    pub fn new(
        simulator: Arc<EvmTransactionSimulator>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            simulator,
            simulation_tasks,
        }
    }

    pub async fn simulate_evm_transaction(
        &self,
        request: EvmSimulationRequest,
    ) -> Result<EvmSimulation, EvmServiceError> {
        let simulator = Arc::clone(&self.simulator);

        self.simulation_tasks
            .run(move || async move {
                simulator
                    .simulate(request)
                    .await
                    .map_err(EvmServiceError::from)
            })
            .await?
    }
}

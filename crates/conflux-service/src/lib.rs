pub mod core_space;

use std::sync::Arc;

use conflux_simulation::core_space::{CoreSpaceSimulationPreparer, CoreSpaceSimulator};
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Clone)]
pub struct ConfluxService {
    core_space_preparer: Arc<CoreSpaceSimulationPreparer>,
    core_space_simulator: Arc<CoreSpaceSimulator>,
    simulation_tasks: SimulationTaskSet,
}

impl ConfluxService {
    pub fn new(
        core_space_preparer: Arc<CoreSpaceSimulationPreparer>,
        core_space_simulator: Arc<CoreSpaceSimulator>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            core_space_preparer,
            core_space_simulator,
            simulation_tasks,
        }
    }

    pub async fn simulate_core_space_transaction(
        &self,
        input: core_space::CoreSpaceSimulationInput,
    ) -> Result<core_space::CoreSpaceSimulation, ConfluxServiceError> {
        let core_space::CoreSpaceSimulationInput { epoch, transaction } = input;
        let preparer = Arc::clone(&self.core_space_preparer);
        let simulator = Arc::clone(&self.core_space_simulator);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let prepared = preparer
                    .prepare_transaction(
                        epoch,
                        transaction.transaction,
                        transaction.storage_limit,
                        transaction.epoch_height,
                    )
                    .await?;

                let simulation = tokio::task::spawn_blocking(move || simulator.simulate(prepared))
                    .await
                    .map_err(|source| ConfluxServiceError::ExecutionTask {
                        space: "Core Space",
                        source,
                    })??;

                Ok::<_, ConfluxServiceError>(simulation)
            })
            .await??;

        Ok(simulation)
    }
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("simulation attempt task failed")]
    AttemptTask {
        #[source]
        source: JoinError,
    },

    #[error("simulation execution failed: {space} blocking execution task failed: {source}")]
    ExecutionTask {
        space: &'static str,
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Simulation(#[from] conflux_simulation::ConfluxSimulationError),
}

impl From<SimulationTaskError> for ConfluxServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

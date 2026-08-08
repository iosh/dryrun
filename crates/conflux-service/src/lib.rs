pub mod core_space;
pub mod espace;

use std::sync::Arc;

use conflux_simulation::{
    core_space::{CoreSpaceSimulationPreparer, CoreSpaceSimulator},
    espace::{EspaceSimulationPreparer, EspaceSimulator},
};
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Clone)]
pub struct ConfluxService {
    espace_preparer: Arc<EspaceSimulationPreparer>,
    espace_simulator: Arc<EspaceSimulator>,
    core_space_preparer: Arc<CoreSpaceSimulationPreparer>,
    core_space_simulator: Arc<CoreSpaceSimulator>,
    simulation_tasks: SimulationTaskSet,
}

impl ConfluxService {
    pub fn new(
        espace_preparer: Arc<EspaceSimulationPreparer>,
        espace_simulator: Arc<EspaceSimulator>,
        core_space_preparer: Arc<CoreSpaceSimulationPreparer>,
        core_space_simulator: Arc<CoreSpaceSimulator>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            espace_preparer,
            espace_simulator,
            core_space_preparer,
            core_space_simulator,
            simulation_tasks,
        }
    }

    pub async fn simulate_espace_transaction(
        &self,
        input: espace::EspaceSimulationInput,
    ) -> Result<espace::EspaceSimulation, ConfluxServiceError> {
        let espace::EspaceSimulationInput { block, transaction } = input;
        let preparer = Arc::clone(&self.espace_preparer);
        let simulator = Arc::clone(&self.espace_simulator);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let prepared = preparer.prepare_transaction(block, transaction).await?;

                let simulation = tokio::task::spawn_blocking(move || simulator.simulate(prepared))
                    .await
                    .map_err(|source| ConfluxServiceError::ExecutionTask {
                        space: "eSpace",
                        source,
                    })??;

                Ok::<_, ConfluxServiceError>(simulation)
            })
            .await??;

        Ok(simulation)
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

impl ConfluxServiceError {
    pub fn rpc_error_code(&self) -> &'static str {
        match self {
            Self::TaskSetClosed => "task_set_closed",
            Self::AttemptTask { .. } => "attempt_task_error",
            Self::ExecutionTask { .. } => "simulation_execution_error",
            Self::Simulation(error) => simulation_error_code(error),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            _ => self.to_string(),
        }
    }
}

impl From<SimulationTaskError> for ConfluxServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

fn simulation_error_code(error: &conflux_simulation::ConfluxSimulationError) -> &'static str {
    use conflux_simulation::ConfluxSimulationError;

    match error {
        ConfluxSimulationError::BlockNotFound { .. } => "block_not_found",
        ConfluxSimulationError::BlockContext(_)
        | ConfluxSimulationError::InvalidBlockContext { .. }
        | ConfluxSimulationError::StateAnchorInconsistent => "block_context_error",
        ConfluxSimulationError::TransactionCompletion { .. } => "transaction_completion_error",
        ConfluxSimulationError::Provider(_) => "rpc_error",
        ConfluxSimulationError::StateAccess { .. } => "state_access_error",
        ConfluxSimulationError::Analysis { .. } => "analysis_failed",
        ConfluxSimulationError::ExecutionInternal { .. } => "simulation_execution_error",
    }
}

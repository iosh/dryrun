use evm_simulation::EvmSimulationError;
use simulation_tasks::SimulationTaskError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum EvmServiceError {
    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("simulation attempt task failed")]
    AttemptTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Simulation(#[from] EvmSimulationError),
}

impl From<SimulationTaskError> for EvmServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

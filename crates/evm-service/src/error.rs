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

impl EvmServiceError {
    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::Simulation(error) if error.is_not_supported())
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::Simulation(error) => error.kind_code(),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            Self::Simulation(error) => error.details().to_owned(),
        }
    }
}

impl From<SimulationTaskError> for EvmServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

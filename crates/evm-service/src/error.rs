use evm_simulation::{EvmPreparationError, EvmSimulationError};
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

    #[error("EVM execution task failed")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Preparation(#[from] EvmPreparationError),

    #[error(transparent)]
    Simulation(#[from] EvmSimulationError),
}

impl EvmServiceError {
    pub fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::Simulation(error) if error.is_not_supported())
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::ExecutionTask { .. } => Some("execution_task_error"),
            Self::Preparation(EvmPreparationError::BlockResolution { .. }) => {
                Some("block_resolution_error")
            }
            Self::Preparation(EvmPreparationError::TransactionCompletion { .. }) => {
                Some("transaction_resolution_error")
            }
            Self::Simulation(error) => error.kind_code(),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            Self::ExecutionTask { .. } => "EVM execution task failed".to_owned(),
            Self::Preparation(error) => error.to_string(),
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

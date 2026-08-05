use evm_simulation::{EvmPreparationError, EvmSimulationError};
use simulation_tasks::SimulationTaskError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum SimulationServiceError {
    #[error("block resolution failed: {details}")]
    BlockResolution { details: String },

    #[error("transaction completion failed: {details}")]
    TransactionCompletion { details: String },

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
    Simulation(#[from] EvmSimulationError),
}

impl SimulationServiceError {
    pub fn block_resolution(details: impl Into<String>) -> Self {
        Self::BlockResolution {
            details: details.into(),
        }
    }

    pub fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::Simulation(error) if error.is_not_supported())
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::BlockResolution { .. } => Some("block_resolution_error"),
            Self::TransactionCompletion { .. } => Some("transaction_resolution_error"),
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::ExecutionTask { .. } => Some("execution_task_error"),
            Self::Simulation(error) => error.kind_code(),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::BlockResolution { details } => details.clone(),
            Self::TransactionCompletion { details } => details.clone(),
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            Self::ExecutionTask { .. } => "EVM execution task failed".to_owned(),
            Self::Simulation(error) => error.details().to_owned(),
        }
    }
}

impl From<SimulationTaskError> for SimulationServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

impl From<EvmPreparationError> for SimulationServiceError {
    fn from(error: EvmPreparationError) -> Self {
        match error {
            EvmPreparationError::BlockResolution { details } => Self::BlockResolution { details },
            EvmPreparationError::TransactionCompletion { details } => {
                Self::TransactionCompletion { details }
            }
        }
    }
}

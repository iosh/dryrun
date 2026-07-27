use evm_engine::{EvmEngineError, TransactionInputError};
use simulation_tasks::SimulationTaskError;
use simulation_transaction::TransactionRequestError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum SimulationServiceError {
    #[error("invalid transaction: {details}")]
    InvalidTransaction { details: String },

    #[error("block resolution failed: {details}")]
    BlockResolution { details: String },

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
    Engine(#[from] EvmEngineError),
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

    fn invalid_transaction(details: impl Into<String>) -> Self {
        Self::InvalidTransaction {
            details: details.into(),
        }
    }

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::Engine(error) if error.is_not_supported())
    }

    pub fn is_invalid_transaction(&self) -> bool {
        matches!(self, Self::InvalidTransaction { .. })
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::InvalidTransaction { .. } => None,
            Self::BlockResolution { .. } => Some("block_resolution_error"),
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::ExecutionTask { .. } => Some("execution_task_error"),
            Self::Engine(error) => error.kind_code(),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::InvalidTransaction { details } => details.clone(),
            Self::BlockResolution { details } => details.clone(),
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            Self::ExecutionTask { .. } => "EVM execution task failed".to_owned(),
            Self::Engine(error) => error.details().to_owned(),
        }
    }
}

impl From<TransactionRequestError> for SimulationServiceError {
    fn from(error: TransactionRequestError) -> Self {
        Self::invalid_transaction(error.to_string())
    }
}

impl From<TransactionInputError> for SimulationServiceError {
    fn from(error: TransactionInputError) -> Self {
        Self::invalid_transaction(error.to_string())
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

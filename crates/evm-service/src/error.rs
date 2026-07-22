use evm_engine::EvmEngineError;
use simulation_tasks::SimulationTaskError;
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum SimulationServiceError {
    #[error("block resolution failed: {details}")]
    BlockResolution { details: String },

    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("timed out waiting for simulation capacity")]
    AdmissionTimedOut,

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

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::Engine(error) if error.is_not_supported())
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::BlockResolution { .. } => Some("block_resolution_error"),
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AdmissionTimedOut => Some("admission_timed_out"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::ExecutionTask { .. } => Some("execution_task_error"),
            Self::Engine(error) => error.kind_code(),
        }
    }

    pub fn details(&self) -> &str {
        match self {
            Self::BlockResolution { details } => details,
            Self::TaskSetClosed => "simulation task set is closed",
            Self::AdmissionTimedOut => "timed out waiting for simulation capacity",
            Self::AttemptTask { .. } => "simulation attempt task failed",
            Self::ExecutionTask { .. } => "EVM execution task failed",
            Self::Engine(error) => error.details(),
        }
    }
}

impl From<SimulationTaskError> for SimulationServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::AdmissionTimedOut => Self::AdmissionTimedOut,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

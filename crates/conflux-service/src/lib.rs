pub mod core_space;
pub mod espace;

use std::sync::Arc;

use conflux_engine::ConfluxEngine;
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Clone)]
pub struct ConfluxService {
    engine: Arc<ConfluxEngine>,
    simulation_tasks: SimulationTaskSet,
}

impl ConfluxService {
    pub fn new(engine: Arc<ConfluxEngine>, simulation_tasks: SimulationTaskSet) -> Self {
        Self {
            engine,
            simulation_tasks,
        }
    }

    pub async fn simulate_espace_transaction(
        &self,
        input: espace::SimulateEspaceTransactionInput,
    ) -> Result<espace::SimulateEspaceTransactionOutput, ConfluxServiceError> {
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move { engine.simulate_espace_transaction(input).await })
            .await??;

        Ok(simulation.into())
    }

    pub async fn simulate_core_space_transaction(
        &self,
        input: core_space::SimulateCoreSpaceTransactionInput,
    ) -> Result<core_space::SimulateCoreSpaceTransactionOutput, ConfluxServiceError> {
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move { engine.simulate_core_space_transaction(input).await })
            .await??;

        Ok(simulation.into())
    }
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("unsupported request: {message}")]
    NotSupported { message: String },

    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("timed out waiting for simulation capacity")]
    AdmissionTimedOut,

    #[error("simulation attempt task failed")]
    AttemptTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Engine(#[from] conflux_engine::ConfluxEngineError),
}

impl ConfluxServiceError {
    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::InvalidRequest { .. } | Self::NotSupported { .. } => None,
            Self::TaskSetClosed => Some("task_set_closed"),
            Self::AdmissionTimedOut => Some("admission_timed_out"),
            Self::AttemptTask { .. } => Some("attempt_task_error"),
            Self::Engine(error) => Some(engine_error_kind(error)),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AdmissionTimedOut => "timed out waiting for simulation capacity".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            _ => self.to_string(),
        }
    }

    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Self::InvalidRequest { .. })
    }

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::NotSupported { .. })
    }
}

impl From<SimulationTaskError> for ConfluxServiceError {
    fn from(error: SimulationTaskError) -> Self {
        match error {
            SimulationTaskError::Closed => Self::TaskSetClosed,
            SimulationTaskError::AdmissionTimedOut => Self::AdmissionTimedOut,
            SimulationTaskError::TaskFailed { source } => Self::AttemptTask { source },
        }
    }
}

fn engine_error_kind(error: &conflux_engine::ConfluxEngineError) -> &'static str {
    use conflux_engine::ConfluxEngineError;

    match error {
        ConfluxEngineError::BlockNotFound { .. } => "block_not_found",
        ConfluxEngineError::BlockContext(_)
        | ConfluxEngineError::InvalidBlockContext { .. }
        | ConfluxEngineError::StateAnchorInconsistent => "block_context_error",
        ConfluxEngineError::RemoteState(_) => "rpc_error",
        ConfluxEngineError::StateAccess { .. } => "state_access_error",
        ConfluxEngineError::ExecutionInternal { .. } => "engine_execution_error",
    }
}

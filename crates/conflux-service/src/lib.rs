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
    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("simulation attempt task failed")]
    AttemptTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Engine(#[from] conflux_engine::ConfluxEngineError),
}

impl ConfluxServiceError {
    pub fn kind_code(&self) -> &'static str {
        match self {
            Self::TaskSetClosed => "task_set_closed",
            Self::AttemptTask { .. } => "attempt_task_error",
            Self::Engine(error) => engine_error_kind(error),
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

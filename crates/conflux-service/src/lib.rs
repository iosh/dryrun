pub mod core_space;
pub mod espace;
mod transaction;

use std::sync::Arc;

use conflux_engine::ConfluxEngine;
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use thiserror::Error;
use tokio::task::JoinError;

pub use transaction::{AccessListItem, ConfluxTransactionRequest};
use transaction::{resolve_core_space_transaction, resolve_espace_transaction};

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
        let espace::SimulateEspaceTransactionInput { block, transaction } = input;
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let context = engine.resolve_espace_context(block).await?;
                let transaction =
                    resolve_espace_transaction(&engine, &context, transaction).await?;
                let prepared = engine
                    .prepare_espace_transaction(context, transaction)
                    .await?;
                let execution_engine = Arc::clone(&engine);

                let simulation = tokio::task::spawn_blocking(move || {
                    execution_engine.simulate_espace_transaction(prepared)
                })
                .await
                .map_err(|source| ConfluxServiceError::ExecutionTask {
                    space: "eSpace",
                    source,
                })??;

                Ok::<_, ConfluxServiceError>(simulation)
            })
            .await??;

        Ok(simulation.into())
    }

    pub async fn simulate_core_space_transaction(
        &self,
        input: core_space::SimulateCoreSpaceTransactionInput,
    ) -> Result<core_space::SimulateCoreSpaceTransactionOutput, ConfluxServiceError> {
        let core_space::SimulateCoreSpaceTransactionInput { epoch, transaction } = input;
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let context = engine.resolve_core_space_context(epoch).await?;
                let transaction =
                    resolve_core_space_transaction(&engine, &context, transaction).await?;
                let prepared = engine
                    .prepare_core_space_transaction(context, transaction)
                    .await?;
                let execution_engine = Arc::clone(&engine);

                let simulation = tokio::task::spawn_blocking(move || {
                    execution_engine.simulate_core_space_transaction(prepared)
                })
                .await
                .map_err(|source| ConfluxServiceError::ExecutionTask {
                    space: "Core Space",
                    source,
                })??;

                Ok::<_, ConfluxServiceError>(simulation)
            })
            .await??;

        Ok(simulation.into())
    }
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("transaction resolution failed: {details}")]
    TransactionResolution { details: String },

    #[error("simulation task set is closed")]
    TaskSetClosed,

    #[error("simulation attempt task failed")]
    AttemptTask {
        #[source]
        source: JoinError,
    },

    #[error("engine execution failed: {space} blocking execution task failed: {source}")]
    ExecutionTask {
        space: &'static str,
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Engine(#[from] conflux_engine::ConfluxEngineError),
}

impl ConfluxServiceError {
    pub(crate) fn transaction_resolution(details: impl Into<String>) -> Self {
        Self::TransactionResolution {
            details: details.into(),
        }
    }

    pub fn kind_code(&self) -> &'static str {
        match self {
            Self::TransactionResolution { .. } => "transaction_resolution_error",
            Self::TaskSetClosed => "task_set_closed",
            Self::AttemptTask { .. } => "attempt_task_error",
            Self::ExecutionTask { .. } => "engine_execution_error",
            Self::Engine(error) => engine_error_kind(error),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TransactionResolution { details } => details.clone(),
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

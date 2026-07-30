pub mod core_space;
pub mod espace;
mod transaction;

use std::sync::Arc;

use conflux_engine::{ConfluxEngine, ConfluxRpcError, HttpConfluxProvider};
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use thiserror::Error;
use tokio::task::JoinError;

pub use transaction::{AccessListItem, ConfluxTransactionRequest};
use transaction::{complete_core_space_transaction, complete_espace_transaction};

#[derive(Clone)]
pub struct ConfluxService {
    provider: Arc<HttpConfluxProvider>,
    engine: Arc<ConfluxEngine>,
    simulation_tasks: SimulationTaskSet,
}

impl ConfluxService {
    pub fn new(
        provider: Arc<HttpConfluxProvider>,
        engine: Arc<ConfluxEngine>,
        simulation_tasks: SimulationTaskSet,
    ) -> Self {
        Self {
            provider,
            engine,
            simulation_tasks,
        }
    }

    pub async fn simulate_espace_transaction(
        &self,
        input: espace::SimulateEspaceTransactionInput,
    ) -> Result<espace::SimulateEspaceTransactionOutput, ConfluxServiceError> {
        let espace::SimulateEspaceTransactionInput { block, transaction } = input;
        let provider = Arc::clone(&self.provider);
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let context = engine.load_espace_context(block).await?;
                let transaction =
                    complete_espace_transaction(&provider, &context, transaction).await?;
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

        Ok(simulation)
    }

    pub async fn simulate_core_space_transaction(
        &self,
        input: core_space::SimulateCoreSpaceTransactionInput,
    ) -> Result<core_space::SimulateCoreSpaceTransactionOutput, ConfluxServiceError> {
        let core_space::SimulateCoreSpaceTransactionInput { epoch, transaction } = input;
        let provider = Arc::clone(&self.provider);
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let context = engine.load_core_space_context(epoch).await?;
                let transaction =
                    complete_core_space_transaction(&provider, &context, transaction).await?;
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

        Ok(simulation)
    }
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("transaction completion failed: {details}")]
    TransactionCompletion { details: String },

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
    Provider(#[from] ConfluxRpcError),

    #[error(transparent)]
    Engine(#[from] conflux_engine::ConfluxEngineError),
}

impl ConfluxServiceError {
    pub(crate) fn transaction_completion(details: impl Into<String>) -> Self {
        Self::TransactionCompletion {
            details: details.into(),
        }
    }

    pub fn rpc_error_code(&self) -> &'static str {
        match self {
            Self::TransactionCompletion { .. } => "transaction_resolution_error",
            Self::TaskSetClosed => "task_set_closed",
            Self::AttemptTask { .. } => "attempt_task_error",
            Self::ExecutionTask { .. } => "engine_execution_error",
            Self::Provider(_) => "rpc_error",
            Self::Engine(error) => engine_error_code(error),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::TransactionCompletion { details } => details.clone(),
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

fn engine_error_code(error: &conflux_engine::ConfluxEngineError) -> &'static str {
    use conflux_engine::ConfluxEngineError;

    match error {
        ConfluxEngineError::BlockNotFound { .. } => "block_not_found",
        ConfluxEngineError::BlockContext(_)
        | ConfluxEngineError::InvalidBlockContext { .. }
        | ConfluxEngineError::StateAnchorInconsistent => "block_context_error",
        ConfluxEngineError::Provider(_) => "rpc_error",
        ConfluxEngineError::StateAccess { .. } => "state_access_error",
        ConfluxEngineError::Analysis { .. } => "analysis_failed",
        ConfluxEngineError::ExecutionInternal { .. } => "engine_execution_error",
    }
}

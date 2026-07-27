pub mod core_space;
pub mod espace;

use std::sync::Arc;

use conflux_engine::{
    ConfluxEngine, TransactionInputError,
    core_space::{CoreSpaceTransaction, SimulateCoreSpaceTransactionInput as EngineCoreSpaceInput},
    espace::{EspaceTransaction, SimulateEspaceTransactionInput as EngineEspaceInput},
};
use simulation_tasks::{SimulationTaskError, SimulationTaskSet};
use simulation_transaction::TransactionRequestError;
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
        let input = build_espace_engine_input(input)?;
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let prepared = engine.prepare_espace_transaction(input).await?;
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
        let input = build_core_space_engine_input(input)?;
        let engine = Arc::clone(&self.engine);
        let simulation = self
            .simulation_tasks
            .run(move || async move {
                let prepared = engine.prepare_core_space_transaction(input).await?;
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

fn build_espace_engine_input(
    input: espace::SimulateEspaceTransactionInput,
) -> Result<EngineEspaceInput, ConfluxServiceError> {
    let espace::SimulateEspaceTransactionInput { block, transaction } = input;
    let transaction = EspaceTransaction::try_from(transaction.complete()?)?;

    Ok(EngineEspaceInput { block, transaction })
}

fn build_core_space_engine_input(
    input: core_space::SimulateCoreSpaceTransactionInput,
) -> Result<EngineCoreSpaceInput, ConfluxServiceError> {
    let core_space::SimulateCoreSpaceTransactionInput { epoch, transaction } = input;
    let core_space::CoreSpaceTransactionRequest {
        transaction,
        storage_limit,
        epoch_height,
    } = transaction;
    let transaction = transaction.complete()?;
    let storage_limit = storage_limit.ok_or_else(|| {
        ConfluxServiceError::invalid_transaction("Core Space transaction storage_limit is required")
    })?;
    let epoch_height = epoch_height.ok_or_else(|| {
        ConfluxServiceError::invalid_transaction("Core Space transaction epoch_height is required")
    })?;
    let transaction =
        CoreSpaceTransaction::try_from_parts(transaction, storage_limit, epoch_height)?;

    Ok(EngineCoreSpaceInput { epoch, transaction })
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("invalid transaction: {details}")]
    InvalidTransaction { details: String },

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
    fn invalid_transaction(details: impl Into<String>) -> Self {
        Self::InvalidTransaction {
            details: details.into(),
        }
    }

    pub fn is_invalid_transaction(&self) -> bool {
        matches!(self, Self::InvalidTransaction { .. })
    }

    pub fn kind_code(&self) -> &'static str {
        match self {
            Self::InvalidTransaction { .. } => "transaction_resolution_error",
            Self::TaskSetClosed => "task_set_closed",
            Self::AttemptTask { .. } => "attempt_task_error",
            Self::ExecutionTask { .. } => "engine_execution_error",
            Self::Engine(error) => engine_error_kind(error),
        }
    }

    pub fn details(&self) -> String {
        match self {
            Self::InvalidTransaction { details } => details.clone(),
            Self::TaskSetClosed => "simulation task set is closed".to_owned(),
            Self::AttemptTask { .. } => "simulation attempt task failed".to_owned(),
            _ => self.to_string(),
        }
    }
}

impl From<TransactionRequestError> for ConfluxServiceError {
    fn from(error: TransactionRequestError) -> Self {
        Self::invalid_transaction(error.to_string())
    }
}

impl From<TransactionInputError> for ConfluxServiceError {
    fn from(error: TransactionInputError) -> Self {
        Self::invalid_transaction(error.to_string())
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

pub mod espace;

use std::sync::Arc;

use conflux_engine::ConfluxEngine;
use thiserror::Error;

#[derive(Clone)]
pub struct ConfluxService {
    engine: Arc<ConfluxEngine>,
}

impl ConfluxService {
    pub fn new(engine: Arc<ConfluxEngine>) -> Self {
        Self { engine }
    }

    pub fn simulate_espace_transaction(
        &self,
        input: espace::SimulateEspaceTransactionInput,
    ) -> Result<espace::SimulateEspaceTransactionOutput, ConfluxServiceError> {
        let engine_input = input.try_into()?;
        let simulation = self.engine.simulate_espace_transaction(engine_input)?;

        Ok(simulation.into())
    }
}

#[derive(Debug, Error)]
pub enum ConfluxServiceError {
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("unsupported request: {message}")]
    NotSupported { message: String },

    #[error(transparent)]
    Engine(#[from] conflux_engine::ConfluxEngineError),
}

impl ConfluxServiceError {
    pub fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::InvalidRequest { .. } | Self::NotSupported { .. } => None,
            Self::Engine(error) => Some(engine_error_kind(error)),
        }
    }

    pub fn details(&self) -> String {
        self.to_string()
    }

    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Self::InvalidRequest { .. })
    }

    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::NotSupported { .. })
    }
}

fn engine_error_kind(error: &conflux_engine::ConfluxEngineError) -> &'static str {
    use conflux_engine::ConfluxEngineError;

    match error {
        ConfluxEngineError::UnsupportedTransactionType { .. } => "not_supported",
        ConfluxEngineError::BlockNotFound { .. } => "block_not_found",
        ConfluxEngineError::BlockContext(_) | ConfluxEngineError::InvalidBlockContext { .. } => {
            "block_context_error"
        }
        ConfluxEngineError::InvalidTransaction { .. } => "invalid_transaction",
        ConfluxEngineError::RemoteState(_) => "rpc_error",
        ConfluxEngineError::StateAccess { .. } => "state_access_error",
        ConfluxEngineError::ExecutionInternal { .. } => "engine_execution_error",
    }
}

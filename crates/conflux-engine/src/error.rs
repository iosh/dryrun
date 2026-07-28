use thiserror::Error;

use crate::{
    ConfluxRpcError,
    execution::{ExecutionBlockContextError, TransactionExecutionError},
};

#[derive(Debug, Error)]
pub enum ConfluxEngineError {
    #[error("block not found: {block}")]
    BlockNotFound { block: String },

    #[error(transparent)]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("block context error: {message}")]
    InvalidBlockContext { message: String },

    #[error("state anchor is inconsistent")]
    StateAnchorInconsistent,

    #[error(transparent)]
    Provider(#[from] ConfluxRpcError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("engine execution failed: {message}")]
    ExecutionInternal { message: String },
}

impl From<TransactionExecutionError> for ConfluxEngineError {
    fn from(error: TransactionExecutionError) -> Self {
        match error {
            TransactionExecutionError::StateAccess(error) => Self::StateAccess {
                message: error.to_string(),
            },
            TransactionExecutionError::MissingObservations => Self::ExecutionInternal {
                message: error.to_string(),
            },
        }
    }
}

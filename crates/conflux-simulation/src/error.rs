use contract_standards::ContractStandardsError;
use thiserror::Error;

use crate::{
    ConfluxRpcError,
    execution::{ExecutionBlockContextError, TransactionExecutionError},
};

#[derive(Debug, Error)]
pub enum ConfluxSimulationError {
    #[error("block not found: {block}")]
    BlockNotFound { block: String },

    #[error(transparent)]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("block context error: {message}")]
    InvalidBlockContext { message: String },

    #[error("state anchor is inconsistent")]
    StateAnchorInconsistent,

    #[error("transaction completion failed: {message}")]
    TransactionCompletion { message: String },

    #[error(transparent)]
    Provider(#[from] ConfluxRpcError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("change analysis failed: {message}")]
    Analysis { message: String },

    #[error("simulation execution failed: {message}")]
    ExecutionInternal { message: String },
}

impl ConfluxSimulationError {
    pub(crate) fn transaction_completion_failed(message: impl Into<String>) -> Self {
        Self::TransactionCompletion {
            message: message.into(),
        }
    }

    pub(crate) fn analysis_failed(message: impl Into<String>) -> Self {
        Self::Analysis {
            message: message.into(),
        }
    }
}

impl From<ContractStandardsError> for ConfluxSimulationError {
    fn from(error: ContractStandardsError) -> Self {
        Self::analysis_failed(error.to_string())
    }
}

impl From<TransactionExecutionError> for ConfluxSimulationError {
    fn from(error: TransactionExecutionError) -> Self {
        match error {
            TransactionExecutionError::BlockContext(error) => Self::BlockContext(error),
            TransactionExecutionError::StateAccess(error) => Self::StateAccess {
                message: error.to_string(),
            },
            error => Self::ExecutionInternal {
                message: error.to_string(),
            },
        }
    }
}

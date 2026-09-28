use alloy::{eips::BlockId, transports::TransportError};
use simulation_core::{CodedError, ErrorCode, LimitExceeded};
use thiserror::Error;
use tokio::task::JoinError;

use crate::StateError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    InvalidInput(String),
    #[error("block {0} was not found")]
    BlockNotFound(BlockId),
    #[error("{0}")]
    Unsupported(String),
    #[error("provider serves chain {actual}, expected {expected}")]
    ChainMismatch { expected: u64, actual: u64 },
    #[error("provider request {operation} failed")]
    Provider {
        operation: &'static str,
        #[source]
        source: TransportError,
    },
    #[error(transparent)]
    State(StateError),
    #[error(transparent)]
    LimitExceeded(LimitExceeded),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("simulation task failed")]
    Runtime(#[source] JoinError),
}

impl From<StateError> for Error {
    fn from(error: StateError) -> Self {
        match error {
            StateError::LimitExceeded(error) => Self::LimitExceeded(error),
            error => Self::State(error),
        }
    }
}

impl From<LimitExceeded> for Error {
    fn from(error: LimitExceeded) -> Self {
        Self::LimitExceeded(error)
    }
}

impl CodedError for Error {
    fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidInput(_) => ErrorCode::InvalidInput,
            Self::BlockNotFound(_) => ErrorCode::ContextNotFound,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::Provider { .. } => ErrorCode::ProviderRequestFailed,
            Self::State(error) => error.code(),
            Self::LimitExceeded(_) => ErrorCode::LimitExceeded,
            Self::Execution(_) => ErrorCode::ExecutionFailed,
            Self::ChainMismatch { .. } | Self::Runtime(_) => ErrorCode::Internal,
        }
    }
}

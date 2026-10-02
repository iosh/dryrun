use alloy::{eips::BlockId, transports::TransportError};
use simulation_core::{CodedError, ErrorCode, LimitExceeded};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    InvalidInput(String),
    #[error("block {0} was not found")]
    BlockNotFound(BlockId),
    #[error("the selected epoch pivot changed during simulation")]
    ContextInconsistent,
    #[error("{0}")]
    Unsupported(String),
    #[error("{endpoint} serves {field} {actual}, expected {expected}")]
    ChainMismatch {
        endpoint: &'static str,
        field: &'static str,
        expected: u64,
        actual: alloy::primitives::U256,
    },
    #[error("Core Space provider request failed")]
    CoreProvider(#[from] conflux_provider::Error),
    #[error("provider request {operation} failed")]
    EspaceProvider {
        operation: &'static str,
        #[source]
        source: TransportError,
    },
    #[error(transparent)]
    State(#[from] StateError),
    #[error(transparent)]
    LimitExceeded(#[from] LimitExceeded),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("{0}")]
    Internal(&'static str),
    #[error("simulation task failed")]
    Runtime(#[source] tokio::task::JoinError),
}

/// State access failures retain their source through the node executor.
#[derive(Debug, Error)]
pub enum StateError {
    #[error("Core Space state request failed")]
    CoreProvider(#[from] conflux_provider::Error),
    #[error("eSpace state request {operation} failed")]
    EspaceProvider {
        operation: &'static str,
        #[source]
        source: TransportError,
    },
    #[error(transparent)]
    LimitExceeded(#[from] LimitExceeded),
    #[error("state is unavailable: {0}")]
    Unavailable(String),
}

impl CodedError for StateError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::CoreProvider(_) | Self::EspaceProvider { .. } => ErrorCode::ProviderRequestFailed,
            Self::LimitExceeded(_) => ErrorCode::LimitExceeded,
            Self::Unavailable(_) => ErrorCode::StateUnavailable,
        }
    }
}

impl CodedError for Error {
    fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidInput(_) => ErrorCode::InvalidInput,
            Self::BlockNotFound(_) => ErrorCode::ContextNotFound,
            Self::ContextInconsistent => ErrorCode::ContextInconsistent,
            Self::Unsupported(_) => ErrorCode::Unsupported,
            Self::CoreProvider(_) | Self::EspaceProvider { .. } => ErrorCode::ProviderRequestFailed,
            Self::State(error) => error.code(),
            Self::LimitExceeded(_) => ErrorCode::LimitExceeded,
            Self::Execution(_) => ErrorCode::ExecutionFailed,
            Self::Internal(_) | Self::ChainMismatch { .. } | Self::Runtime(_) => {
                ErrorCode::Internal
            }
        }
    }
}

impl From<cfx_statedb::Error> for Error {
    fn from(error: cfx_statedb::Error) -> Self {
        match error {
            cfx_statedb::Error::Storage(cfx_storage::Error::External(source)) => {
                match source.downcast::<StateError>() {
                    Ok(error) => Self::State(*error),
                    Err(error) => Self::Execution(error.to_string()),
                }
            }
            error => Self::Execution(error.to_string()),
        }
    }
}

impl From<StateError> for cfx_storage::Error {
    fn from(error: StateError) -> Self {
        Self::External(Box::new(error))
    }
}

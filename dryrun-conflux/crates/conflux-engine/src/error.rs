use thiserror::Error;

use crate::{execution::ExecutionBlockContextError, state::RemoteStateProviderError};

#[derive(Debug, Error)]
pub enum ConfluxEngineError {
    #[error("block not found: {block}")]
    BlockNotFound { block: String },

    #[error(transparent)]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("block context error: {message}")]
    InvalidBlockContext { message: String },

    #[error("invalid transaction: {message}")]
    InvalidTransaction { message: String },

    #[error("unsupported transaction type: {tx_type}")]
    UnsupportedTransactionType { tx_type: &'static str },

    #[error(transparent)]
    RemoteState(#[from] RemoteStateProviderError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("engine execution failed: {message}")]
    ExecutionInternal { message: String },
}

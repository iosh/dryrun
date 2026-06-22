use thiserror::Error;

use crate::{execution::ExecutionBlockContextError, state::RemoteStateProviderError};

#[derive(Debug, Error)]
pub enum ConfluxEngineError {
    #[error("unsupported eSpace block selector: {selector}")]
    UnsupportedBlockSelector { selector: &'static str },

    #[error("eSpace block not found: {block}")]
    BlockNotFound { block: String },

    #[error(transparent)]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("block context error: {message}")]
    InvalidBlockContext { message: String },

    #[error("invalid eSpace transaction: {message}")]
    InvalidTransaction { message: String },

    #[error("unsupported eSpace transaction type: {tx_type}")]
    UnsupportedTransactionType { tx_type: &'static str },

    #[error(transparent)]
    RemoteState(#[from] RemoteStateProviderError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("engine execution failed: {message}")]
    ExecutionInternal { message: String },

    #[error("unexpected engine error: {message}")]
    Unexpected { message: String },
}

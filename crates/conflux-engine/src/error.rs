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

    #[error("state anchor is inconsistent")]
    StateAnchorInconsistent,

    #[error(transparent)]
    RemoteState(#[from] RemoteStateProviderError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("engine execution failed: {message}")]
    ExecutionInternal { message: String },
}

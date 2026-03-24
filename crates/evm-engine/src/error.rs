use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvmEngineError {
    #[error("{0}")]
    NotReady(String),

    #[error("{0}")]
    Internal(String),
}

impl EvmEngineError {
    pub fn not_ready(details: impl Into<String>) -> Self {
        Self::NotReady(details.into())
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::Internal(details.into())
    }
}

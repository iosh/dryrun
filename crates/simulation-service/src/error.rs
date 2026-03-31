use evm_engine::EvmEngineError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimulationServiceError {
    #[error("{0}")]
    NotSupported(String),

    #[error("{details}")]
    Internal {
        subkind: &'static str,
        details: String,
    },
}

impl From<EvmEngineError> for SimulationServiceError {
    fn from(error: EvmEngineError) -> Self {
        match error {
            EvmEngineError::NotSupported(details) => Self::NotSupported(details),
            EvmEngineError::Internal { subkind, details } => Self::Internal { subkind, details },
        }
    }
}

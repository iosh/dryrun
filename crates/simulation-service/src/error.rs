use evm_engine::EvmEngineError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimulationServiceError {
    #[error("{0}")]
    NotReady(String),

    #[error("{0}")]
    Internal(String),
}

impl From<EvmEngineError> for SimulationServiceError {
    fn from(error: EvmEngineError) -> Self {
        match error {
            EvmEngineError::NotReady(details) => Self::NotReady(details),
            EvmEngineError::Internal(details) => Self::Internal(details),
        }
    }
}

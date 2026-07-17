use evm_engine::EvmEngineError;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub struct SimulationServiceError(#[from] EvmEngineError);

impl SimulationServiceError {
    pub fn is_not_supported(&self) -> bool {
        self.0.is_not_supported()
    }

    pub fn kind_code(&self) -> Option<&'static str> {
        self.0.kind_code()
    }

    pub fn details(&self) -> &str {
        self.0.details()
    }
}

#[cfg(test)]
mod tests {
    use evm_engine::EvmEngineError;

    use super::SimulationServiceError;

    #[test]
    fn wrapper_delegates_internal_error_classification() {
        let error = SimulationServiceError::from(EvmEngineError::state_access_error(
            "missing account state",
        ));

        assert!(!error.is_not_supported());
        assert_eq!(error.kind_code(), Some("state_access_error"));
        assert_eq!(error.details(), "missing account state");
    }

    #[test]
    fn wrapper_delegates_not_supported_error_classification() {
        let error = SimulationServiceError::from(EvmEngineError::not_supported(
            "block.hash is not supported yet",
        ));

        assert!(error.is_not_supported());
        assert_eq!(error.kind_code(), None);
        assert_eq!(error.details(), "block.hash is not supported yet");
    }
}

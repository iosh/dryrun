use jsonrpsee::types::ErrorObjectOwned;
use simulation_core::error::{Diagnostic, ErrorCode, ErrorInfo};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    InvalidParams(String),

    #[error("{0}")]
    NotSupported(String),
}

impl ValidationError {
    pub(crate) fn invalid_params(details: impl Into<String>) -> Self {
        Self::InvalidParams(details.into())
    }

    pub(crate) fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }
}

impl From<ValidationError> for ErrorObjectOwned {
    fn from(error: ValidationError) -> Self {
        let code = match &error {
            ValidationError::InvalidParams(_) => ErrorCode::InvalidInput,
            ValidationError::NotSupported(_) => ErrorCode::UnsupportedSimulation,
        };
        rpc_error(Diagnostic::new(code, error.to_string()))
    }
}

pub(crate) fn rpc_error(error: impl ErrorInfo + std::fmt::Debug) -> ErrorObjectOwned {
    let diagnostic = error.diagnostic();
    tracing::warn!(error = ?error, code = ?diagnostic.code, "simulation request failed");
    let (code, message, data) = simulation_core::codec::json_rpc_error(diagnostic);
    ErrorObjectOwned::owned(code, message, Some(data))
}

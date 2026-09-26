use jsonrpsee::types::ErrorObjectOwned;
use simulation_core::error::{Diagnostic, ErrorCode, ErrorInfo};

#[derive(Debug, thiserror::Error)]
pub(super) enum ValidationError {
    #[error("{0}")]
    InvalidParams(String),

    #[error("{0}")]
    NotSupported(String),
}

impl ValidationError {
    pub(super) fn invalid_params(details: impl Into<String>) -> Self {
        Self::InvalidParams(details.into())
    }

    pub(super) fn not_supported(details: impl Into<String>) -> Self {
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

pub(super) fn rpc_error(error: impl ErrorInfo + std::fmt::Debug) -> ErrorObjectOwned {
    let diagnostic = error.diagnostic();
    tracing::warn!(error = ?error, code = ?diagnostic.code, "simulation request failed");
    let (code, message, data) = simulation_core::codec::json_rpc_error(diagnostic);
    ErrorObjectOwned::owned(code, message, Some(data))
}

pub(super) fn invalid_params(message: impl Into<String>) -> ErrorObjectOwned {
    rpc_error(Diagnostic::new(ErrorCode::InvalidInput, message))
}

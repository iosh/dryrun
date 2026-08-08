use conflux_service::ConfluxServiceError;
use jsonrpsee::types::{
    ErrorObjectOwned,
    error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE},
};
use tracing::error;

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
        match error {
            ValidationError::InvalidParams(details) => invalid_params(details),
            ValidationError::NotSupported(details) => not_supported(details),
        }
    }
}

pub(super) fn invalid_params(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INVALID_PARAMS_CODE, details.into(), None::<()>)
}

fn not_supported(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(-32004, details.into(), None::<()>)
}

fn internal_error() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INTERNAL_ERROR_CODE, "Internal error", None::<()>)
}

pub(super) fn core_space_response_mapping_error(details: impl Into<String>) -> ErrorObjectOwned {
    let details = details.into();
    error!(details, "Conflux Core Space response mapping failed");
    internal_error()
}

pub(super) fn map_core_space_service_error(error: ConfluxServiceError) -> ErrorObjectOwned {
    error!(error = ?error, "Conflux Core Space simulation failed");
    internal_error()
}

pub(super) fn map_espace_service_error(error: ConfluxServiceError) -> ErrorObjectOwned {
    error!(error = ?error, "Conflux eSpace simulation failed");
    internal_error()
}

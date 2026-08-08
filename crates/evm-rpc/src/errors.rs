use jsonrpsee::types::{
    ErrorObjectOwned,
    error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE},
};
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
        match error {
            ValidationError::InvalidParams(details) => {
                ErrorObjectOwned::owned(INVALID_PARAMS_CODE, details, None::<()>)
            }
            ValidationError::NotSupported(details) => {
                ErrorObjectOwned::owned(-32004, details, None::<()>)
            }
        }
    }
}

pub(crate) fn internal_error() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INTERNAL_ERROR_CODE, "Internal error", None::<()>)
}

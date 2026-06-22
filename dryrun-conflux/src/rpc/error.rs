use conflux_service::ConfluxServiceError;
use jsonrpsee::types::{
    ErrorObjectOwned,
    error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE},
};
use serde::Serialize;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ErrorData {
    #[serde(skip_serializing_if = "Option::is_none")]
    subkind: Option<&'static str>,
    details: String,
}

pub(super) fn invalid_params(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INVALID_PARAMS_CODE,
        "Invalid params",
        Some(ErrorData {
            subkind: None,
            details: details.into(),
        }),
    )
}

fn not_supported(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        -32004,
        "Not supported",
        Some(ErrorData {
            subkind: None,
            details: details.into(),
        }),
    )
}

fn internal_error(kind_code: Option<&'static str>, details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INTERNAL_ERROR_CODE,
        "Internal error",
        Some(ErrorData {
            subkind: kind_code,
            details: details.into(),
        }),
    )
}

pub(super) fn map_service_error(error: ConfluxServiceError) -> ErrorObjectOwned {
    let details = error.details();

    if error.is_invalid_request() {
        invalid_params(details)
    } else if error.is_not_supported() {
        not_supported(details)
    } else {
        internal_error(error.kind_code(), details)
    }
}

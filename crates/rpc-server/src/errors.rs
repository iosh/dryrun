use jsonrpsee::types::{ErrorObjectOwned, error::INVALID_PARAMS_CODE};
use serde::Serialize;

#[derive(Debug)]
pub(crate) enum ValidationError {
    InvalidParams(String),
    NotSupported(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ErrorData {
    kind: &'static str,
    details: String,
}

impl ValidationError {
    pub(crate) fn invalid_params(details: impl Into<String>) -> Self {
        Self::InvalidParams(details.into())
    }

    pub(crate) fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }

    pub(crate) fn into_error_object(self) -> ErrorObjectOwned {
        match self {
            Self::InvalidParams(details) => ErrorObjectOwned::owned(
                INVALID_PARAMS_CODE,
                "Invalid params",
                Some(ErrorData {
                    kind: "invalid_params",
                    details,
                }),
            ),
            Self::NotSupported(details) => ErrorObjectOwned::owned(
                -32004,
                "Not supported",
                Some(ErrorData {
                    kind: "not_supported",
                    details,
                }),
            ),
        }
    }
}

pub(crate) fn not_ready(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        -32010,
        "Method not ready",
        Some(ErrorData {
            kind: "not_ready",
            details: details.into(),
        }),
    )
}

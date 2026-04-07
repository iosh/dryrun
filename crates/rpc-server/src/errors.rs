use jsonrpsee::types::{
    ErrorObjectOwned,
    error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0}")]
    InvalidParams(String),

    #[error("{0}")]
    NotSupported(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct ErrorData {
    #[serde(skip_serializing_if = "Option::is_none")]
    subkind: Option<&'static str>,
    details: String,
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
            ValidationError::InvalidParams(details) => ErrorObjectOwned::owned(
                INVALID_PARAMS_CODE,
                "Invalid params",
                Some(ErrorData {
                    subkind: None,
                    details,
                }),
            ),
            ValidationError::NotSupported(details) => ErrorObjectOwned::owned(
                -32004,
                "Not supported",
                Some(ErrorData {
                    subkind: None,
                    details,
                }),
            ),
        }
    }
}

pub(crate) fn internal_error(
    kind_code: Option<&'static str>,
    details: impl Into<String>,
) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INTERNAL_ERROR_CODE,
        "Internal error",
        Some(ErrorData {
            subkind: kind_code,
            details: details.into(),
        }),
    )
}

pub(crate) fn not_supported(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        -32004,
        "Not supported",
        Some(ErrorData {
            subkind: None,
            details: details.into(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::to_value;

    use super::internal_error;

    #[test]
    fn internal_error_serializes_subkind_and_details() {
        let error = internal_error(Some("runtime_error"), "execution task panicked");

        let value = to_value(error).expect("rpc error should serialize");
        assert_eq!(value["code"], -32603);
        assert_eq!(value["message"], "Internal error");
        assert_eq!(value["data"]["subkind"], "runtime_error");
        assert_eq!(value["data"]["details"], "execution task panicked");
    }
}

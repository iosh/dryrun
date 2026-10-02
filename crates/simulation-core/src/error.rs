use serde::Serialize;

/// Stable public identity of an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ErrorCode {
    #[serde(rename = "input.invalid")]
    InvalidInput,
    #[serde(rename = "context.not_found")]
    ContextNotFound,
    #[serde(rename = "context.inconsistent")]
    ContextInconsistent,
    #[serde(rename = "simulation.unsupported")]
    Unsupported,
    #[serde(rename = "provider.request_failed")]
    ProviderRequestFailed,
    #[serde(rename = "state.unavailable")]
    StateUnavailable,
    #[serde(rename = "limit.exceeded")]
    LimitExceeded,
    #[serde(rename = "execution.failed")]
    ExecutionFailed,
    #[serde(rename = "service.closed")]
    ServiceClosed,
    #[serde(rename = "service.timeout")]
    ServiceTimeout,
    #[serde(rename = "service.cancelled")]
    ServiceCancelled,
    #[serde(rename = "internal.error")]
    Internal,
}

/// An error with a public code. Its `Display` is the public message, so it
/// must not include upstream details; those stay in `source()`.
pub trait CodedError: std::error::Error {
    fn code(&self) -> ErrorCode;

    fn to_object(&self) -> ErrorObject {
        ErrorObject {
            code: self.code(),
            message: self.to_string(),
        }
    }
}

/// The public form of an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorObject {
    pub code: ErrorCode,
    pub message: String,
}

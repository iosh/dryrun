use std::fmt::Debug;

use jsonrpsee::types::ErrorObjectOwned;
use serde::Serialize;
use simulation_core::{CodedError, ErrorCode};

use crate::tasks::SimulationTaskError;

/// The JSON-RPC error of a failed request. `data.code` carries the stable
/// error code.
pub(super) fn rpc_error<E: CodedError + Debug>(error: &E) -> ErrorObjectOwned {
    let code = error.code();
    tracing::warn!(?error, ?code, "simulation request failed");
    rpc_error_object(code, error.to_string())
}

/// Parameter errors reported by jsonrpsee, recoded as `input.invalid`.
pub(super) fn invalid_params(error: ErrorObjectOwned) -> ErrorObjectOwned {
    let message = error
        .data()
        .and_then(|data| serde_json::from_str::<String>(data.get()).ok())
        .unwrap_or_else(|| error.message().to_owned());
    rpc_error_object(ErrorCode::InvalidInput, message)
}

pub(super) fn rpc_error_object(code: ErrorCode, message: String) -> ErrorObjectOwned {
    #[derive(Serialize)]
    struct Data {
        code: ErrorCode,
    }

    ErrorObjectOwned::owned(json_rpc_code(code), message, Some(Data { code }))
}

fn json_rpc_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::InvalidInput => -32602,
        ErrorCode::ContextNotFound => -32001,
        ErrorCode::ContextInconsistent => -32003,
        ErrorCode::Unsupported => -32004,
        ErrorCode::ServiceClosed => -32005,
        ErrorCode::ServiceTimeout => -32006,
        ErrorCode::ServiceCancelled => -32007,
        ErrorCode::ProviderRequestFailed => -32008,
        ErrorCode::StateUnavailable => -32009,
        ErrorCode::LimitExceeded => -32010,
        ErrorCode::ExecutionFailed | ErrorCode::Internal => -32603,
    }
}

impl CodedError for SimulationTaskError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::Closed => ErrorCode::ServiceClosed,
            Self::ResponseTimedOut => ErrorCode::ServiceTimeout,
            Self::TaskCancelled { .. } => ErrorCode::ServiceCancelled,
            Self::TaskPanicked { .. } => ErrorCode::Internal,
        }
    }
}

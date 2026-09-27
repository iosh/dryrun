use std::fmt::Debug;

use jsonrpsee::types::ErrorObjectOwned;
use simulation_core::error::{Diagnostic, ErrorCode, ErrorInfo};

use crate::simulation_tasks::SimulationTaskError;

pub(super) fn rpc_error(error: impl ErrorInfo + Debug) -> ErrorObjectOwned {
    let diagnostic = error.diagnostic();
    tracing::warn!(error = ?error, code = ?diagnostic.code, "simulation request failed");
    let (code, message, data) = simulation_core::codec::json_rpc_error(diagnostic);
    ErrorObjectOwned::owned(code, message, Some(data))
}

pub(super) fn invalid_params(error: ErrorObjectOwned) -> ErrorObjectOwned {
    let message = error
        .data()
        .and_then(|data| serde_json::from_str::<String>(data.get()).ok())
        .unwrap_or_else(|| error.message().to_owned());
    rpc_error(Diagnostic::new(ErrorCode::InvalidInput, message))
}

impl ErrorInfo for SimulationTaskError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Closed => ErrorCode::ServiceClosed,
            Self::ResponseTimedOut => ErrorCode::ServiceTimeout,
            Self::TaskCancelled { .. } => ErrorCode::ServiceCancelled,
            Self::TaskPanicked { .. } => ErrorCode::Internal,
        }
        .diagnostic()
    }
}

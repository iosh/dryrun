use serde::Serialize;

use crate::error::{Diagnostic, DiagnosticData, ErrorCode};

#[derive(Serialize)]
pub struct JsonRpcErrorData {
    code: ErrorCode,
    #[serde(flatten)]
    details: Option<DiagnosticData>,
}

/// Encodes the common diagnostic into the JSON-RPC error object's three fields.
pub fn json_rpc_error(diagnostic: Diagnostic) -> (i32, String, JsonRpcErrorData) {
    let code = match diagnostic.code {
        ErrorCode::InvalidInput => -32602,
        ErrorCode::ContextNotFound => -32001,
        ErrorCode::CompletionFailed => -32002,
        ErrorCode::InconsistentContext | ErrorCode::ContextUnavailable => -32003,
        ErrorCode::UnsupportedSimulation => -32004,
        ErrorCode::ServiceClosed => -32005,
        ErrorCode::ServiceTimeout => -32006,
        ErrorCode::ServiceCancelled => -32007,
        ErrorCode::ProviderRequestFailed => -32008,
        ErrorCode::StateUnavailable => -32009,
        _ => -32603,
    };
    (
        code,
        diagnostic.message,
        JsonRpcErrorData {
            code: diagnostic.code,
            details: diagnostic.data,
        },
    )
}

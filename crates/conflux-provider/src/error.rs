use alloy_json_rpc::RpcError;
use alloy_transport::{TransportError, TransportErrorKind};
use thiserror::Error;

use crate::AddressError;

#[derive(Debug, Error)]
pub enum ConfluxProviderError {
    #[error("transport error for {method}: {source}")]
    Transport {
        method: &'static str,
        #[source]
        source: TransportErrorKind,
    },
    #[error("JSON-RPC error for {method}: code {code}, {message}{data}")]
    JsonRpc {
        method: &'static str,
        code: i64,
        message: String,
        data: String,
    },
    #[error("parameter encoding error for {method}: {source}")]
    ParameterEncoding {
        method: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("response decoding error for {method}: {detail}")]
    ResponseDecoding {
        method: &'static str,
        detail: String,
    },
    #[error("Core address error for {method}: {source}")]
    Address {
        method: &'static str,
        #[source]
        source: AddressError,
    },
    #[error("typed batch protocol error for {batch} method {method}: {detail}")]
    BatchProtocol {
        batch: &'static str,
        method: &'static str,
        detail: String,
    },
    #[error("local RPC client error for {method}: {detail}")]
    ClientUsage {
        method: &'static str,
        detail: String,
    },
}

pub(crate) fn classify_alloy_rpc_error(
    method: &'static str,
    error: TransportError,
) -> ConfluxProviderError {
    match error {
        RpcError::ErrorResp(payload) => ConfluxProviderError::JsonRpc {
            method,
            code: payload.code,
            message: payload.message.into_owned(),
            data: payload
                .data
                .map(|data| data.get().to_owned())
                .unwrap_or_default(),
        },
        RpcError::NullResp => ConfluxProviderError::ResponseDecoding {
            method,
            detail: "server returned null".to_owned(),
        },
        RpcError::UnsupportedFeature(detail) => ConfluxProviderError::ClientUsage {
            method,
            detail: (*detail).to_owned(),
        },
        RpcError::LocalUsageError(error) => ConfluxProviderError::ClientUsage {
            method,
            detail: error.to_string(),
        },
        RpcError::SerError(error) => ConfluxProviderError::ParameterEncoding {
            method,
            source: error,
        },
        RpcError::DeserError { err, text } => ConfluxProviderError::ResponseDecoding {
            method,
            detail: format!("{err}; response={text}"),
        },
        RpcError::Transport(source) => ConfluxProviderError::Transport { method, source },
    }
}

pub(crate) fn classify_batch_error(
    batch: &'static str,
    method: &'static str,
    error: TransportError,
) -> ConfluxProviderError {
    if let RpcError::Transport(TransportErrorKind::MissingBatchResponse(id)) = &error {
        return ConfluxProviderError::BatchProtocol {
            batch,
            method,
            detail: format!("missing response for request id {id}"),
        };
    }
    classify_alloy_rpc_error(method, error)
}

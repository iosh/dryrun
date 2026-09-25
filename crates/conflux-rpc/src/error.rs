use conflux_simulation::{
    ConfluxRpcError, ConfluxStateAnchorError,
    core_space::{CoreSpaceContextError, CoreSpaceSimulationError},
    espace::{EspaceContextError, EspaceSimulationError, EspaceTransactionCompletionError},
};
use jsonrpsee::types::{
    ErrorObjectOwned,
    error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE},
};
use simulation_tasks::SimulationTaskError;
use tracing::{error, warn};

const CONTEXT_NOT_FOUND_CODE: i32 = -32001;
const TRANSACTION_COMPLETION_FAILED_CODE: i32 = -32002;
const SIMULATION_CLOSED_CODE: i32 = -32005;
const SIMULATION_TIMEOUT_CODE: i32 = -32006;
const SIMULATION_CANCELLED_CODE: i32 = -32007;

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

fn context_not_found(details: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(CONTEXT_NOT_FOUND_CODE, details.into(), None::<()>)
}

fn transaction_completion_failed(details: &'static str) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(TRANSACTION_COMPLETION_FAILED_CODE, details, None::<()>)
}

fn inconsistent_context(message: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        -32003,
        message.into(),
        Some(serde_json::json!({"code": "context.inconsistent"})),
    )
}

fn unavailable_context(message: impl Into<String>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        -32003,
        message.into(),
        Some(serde_json::json!({"code": "context.unavailable"})),
    )
}

fn provider_error_response(error: ConfluxRpcError) -> ErrorObjectOwned {
    let message = match error {
        ConfluxRpcError::AddressEncoding { .. } => return internal_error(),
        ConfluxRpcError::Core { .. } => {
            "Simulation service could not query the upstream Core Space node"
        }
        ConfluxRpcError::Espace { .. } => {
            "Simulation service could not query the upstream eSpace node"
        }
        ConfluxRpcError::InvalidResponse { .. } => {
            "Upstream chain node returned invalid response data"
        }
    };
    ErrorObjectOwned::owned(
        -32008,
        message,
        Some(serde_json::json!({"code": "provider.request_failed"})),
    )
}

fn state_anchor_error_response(error: ConfluxStateAnchorError) -> ErrorObjectOwned {
    match error {
        ConfluxStateAnchorError::Rpc(error) => provider_error_response(error),
        ConfluxStateAnchorError::Mismatch { .. } => inconsistent_context(error.to_string()),
        _ => internal_error(),
    }
}

pub(super) fn core_space_response_error(details: impl Into<String>) -> ErrorObjectOwned {
    let details = details.into();
    error!(details, "Conflux Core Space response mapping failed");
    internal_error()
}

pub(super) fn core_space_error_response(error: CoreSpaceSimulationError) -> ErrorObjectOwned {
    match error {
        CoreSpaceSimulationError::Input(error) => invalid_params(error.to_string()),
        CoreSpaceSimulationError::Context(error) => {
            warn!(error = ?error, "Conflux Core Space context failed");
            match error {
                CoreSpaceContextError::Rpc(error) => provider_error_response(error),
                CoreSpaceContextError::StateAnchor(error) => state_anchor_error_response(error),
                CoreSpaceContextError::PivotBlockNotFound { .. }
                | CoreSpaceContextError::EspaceBlockNotFound { .. } => {
                    context_not_found(error.to_string())
                }
                CoreSpaceContextError::SelectedBlockIsNotPivot { .. } => {
                    inconsistent_context(error.to_string())
                }
                CoreSpaceContextError::BlockContext(error) => {
                    unavailable_context(error.to_string())
                }
                CoreSpaceContextError::ConsensusContextUnavailable { .. } => {
                    unavailable_context(error.to_string())
                }
                _ => internal_error(),
            }
        }
        CoreSpaceSimulationError::Completion(error) => {
            warn!(error = ?error, "Conflux Core Space transaction completion failed");
            transaction_completion_failed("Unable to complete the transaction")
        }
        error => {
            error!(error = ?error, "Conflux Core Space simulation failed");
            internal_error()
        }
    }
}

pub(super) fn espace_error_response(error: EspaceSimulationError) -> ErrorObjectOwned {
    match error {
        EspaceSimulationError::Input(error) => invalid_params(error.to_string()),
        EspaceSimulationError::Context(error) => {
            warn!(error = ?error, "Conflux eSpace context failed");
            match error {
                EspaceContextError::Rpc(error) => provider_error_response(error),
                EspaceContextError::StateAnchor(error) => state_anchor_error_response(error),
                EspaceContextError::EspaceBlockNotFound { .. }
                | EspaceContextError::CoreSpacePivotNotFound { .. } => {
                    context_not_found(error.to_string())
                }
                EspaceContextError::CoreSpacePivotMismatch { .. } => {
                    inconsistent_context(error.to_string())
                }
                EspaceContextError::BlockContext(error) => unavailable_context(error.to_string()),
                _ => internal_error(),
            }
        }
        EspaceSimulationError::Completion(
            error @ EspaceTransactionCompletionError::UnsupportedTransactionType { .. },
        ) => ValidationError::not_supported(error.to_string()).into(),
        EspaceSimulationError::Completion(error) => {
            warn!(error = ?error, "Conflux eSpace transaction completion failed");
            transaction_completion_failed(transaction_completion_message(&error))
        }
        error => {
            error!(error = ?error, "Conflux eSpace simulation failed");
            internal_error()
        }
    }
}

fn transaction_completion_message(error: &EspaceTransactionCompletionError) -> &'static str {
    match error {
        EspaceTransactionCompletionError::NonceLookup { .. } => {
            "Unable to resolve the sender nonce; provide transaction.nonce explicitly"
        }
        EspaceTransactionCompletionError::GasEstimation { .. } => {
            "Unable to estimate transaction gas; provide transaction.gas explicitly"
        }
        EspaceTransactionCompletionError::GasPriceSuggestion { .. } => {
            "Unable to suggest a gas price; provide transaction.gasPrice explicitly"
        }
        EspaceTransactionCompletionError::PriorityFeeSuggestion { .. } => {
            "Unable to suggest a priority fee; provide transaction.maxPriorityFeePerGas explicitly"
        }
        _ => "Unable to complete the transaction",
    }
}

pub(super) fn simulation_task_error_response(error: SimulationTaskError) -> ErrorObjectOwned {
    match error {
        SimulationTaskError::Closed => {
            error!("Conflux simulation task admission is closed");
            ErrorObjectOwned::owned(
                SIMULATION_CLOSED_CODE,
                "Simulation service is closing",
                None::<()>,
            )
        }
        SimulationTaskError::ResponseTimedOut => {
            warn!("Conflux simulation response deadline exceeded");
            ErrorObjectOwned::owned(
                SIMULATION_TIMEOUT_CODE,
                "Simulation response timed out",
                None::<()>,
            )
        }
        SimulationTaskError::TaskCancelled { source } => {
            warn!(error = ?source, "Conflux simulation task was cancelled");
            ErrorObjectOwned::owned(
                SIMULATION_CANCELLED_CODE,
                "Simulation task was cancelled",
                None::<()>,
            )
        }
        SimulationTaskError::TaskPanicked { source } => {
            error!(error = ?source, "Conflux simulation task panicked");
            internal_error()
        }
    }
}

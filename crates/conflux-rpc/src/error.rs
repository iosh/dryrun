use conflux_simulation::{
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

pub(super) fn core_space_response_error(details: impl Into<String>) -> ErrorObjectOwned {
    let details = details.into();
    error!(details, "Conflux Core Space response mapping failed");
    internal_error()
}

pub(super) fn core_space_error_response(error: CoreSpaceSimulationError) -> ErrorObjectOwned {
    match error {
        CoreSpaceSimulationError::Input(error) => invalid_params(error.to_string()),
        CoreSpaceSimulationError::Context(
            error @ (CoreSpaceContextError::PivotBlockNotFound { .. }
            | CoreSpaceContextError::EspaceBlockNotFound { .. }),
        ) => context_not_found(error.to_string()),
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
        EspaceSimulationError::Context(error @ EspaceContextError::EspaceBlockNotFound { .. }) => {
            context_not_found(error.to_string())
        }
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
    error!(error = ?error, "Conflux simulation task failed");
    internal_error()
}

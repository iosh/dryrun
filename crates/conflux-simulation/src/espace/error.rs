use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use simulation_core::error::{Diagnostic, ErrorCode as Code, ErrorInfo, contract_diagnostic};
use simulation_core::observation::AnalysisLimitExceeded;
use std::error::Error as StdError;
use thiserror::Error;

use super::{EspaceContextError, EspaceTransactionInputError, TxType};
use crate::{
    ConfluxRpcError,
    error::{estimation_diagnostic, source_diagnostic},
};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceTransactionCompletionError {
    #[error("eSpace does not support {transaction_type} transactions")]
    UnsupportedTransactionType { transaction_type: TxType },

    #[error("failed to fetch the sender nonce at eSpace block {block_number}: {source}")]
    NonceLookup {
        block_number: u64,
        #[source]
        source: ConfluxRpcError,
    },
    #[error("failed to estimate transaction gas at eSpace block {block_number}: {source}")]
    GasEstimation {
        block_number: u64,
        #[source]
        source: ConfluxRpcError,
    },
    #[error("gas estimate at eSpace block {block_number} exceeds u64: {value}")]
    GasEstimateOutOfRange { block_number: u64, value: U256 },
    #[error("failed to fetch the suggested eSpace gas price: {source}")]
    GasPriceSuggestion {
        #[source]
        source: ConfluxRpcError,
    },
    #[error("failed to fetch the suggested eSpace max priority fee per gas: {source}")]
    PriorityFeeSuggestion {
        #[source]
        source: ConfluxRpcError,
    },
    #[error("calculated eSpace max fee per gas exceeds U256")]
    MaxFeePerGasOverflow,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceStateAccessError {
    #[error("failed to prepare anchored Conflux state: {source}")]
    Preparation {
        #[source]
        source: StorageError,
    },
    #[error("failed to initialize anchored Conflux state: {source}")]
    Initialization {
        #[source]
        source: StateDbError,
    },
    #[error("eSpace state access failed during {operation}: {source}")]
    Operation {
        operation: &'static str,
        #[source]
        source: StateDbError,
    },
}

#[derive(Debug, Error)]
#[error("eSpace execution result could not be integrated: {details}")]
pub struct EspaceResultIntegrationError {
    details: String,
}

impl EspaceResultIntegrationError {
    pub(crate) fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }

    pub(crate) fn invalid_executor_output(details: impl Into<String>) -> Self {
        Self::new(format!(
            "executor returned an invalid result: {}",
            details.into()
        ))
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceExecutionError {
    #[error(transparent)]
    StateAccess(#[from] EspaceStateAccessError),
    #[error(transparent)]
    ResultIntegration(#[from] EspaceResultIntegrationError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceSimulationError {
    #[error(transparent)]
    Input(#[from] EspaceTransactionInputError),
    #[error(transparent)]
    Context(#[from] EspaceContextError),
    #[error(transparent)]
    Completion(#[from] EspaceTransactionCompletionError),
    #[error(transparent)]
    Execution(#[from] EspaceExecutionError),
    #[error(transparent)]
    Runtime(#[from] simulation_core::simulation::RuntimeError),
}

impl ErrorInfo for EspaceTransactionCompletionError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::UnsupportedTransactionType { .. } => Code::UnsupportedSimulation.diagnostic(),
            Self::GasEstimation { source, .. } => estimation_diagnostic(source),
            Self::NonceLookup { source, .. }
            | Self::GasPriceSuggestion { source }
            | Self::PriorityFeeSuggestion { source } => source.diagnostic(),
            Self::GasEstimateOutOfRange { .. } | Self::MaxFeePerGasOverflow => {
                Code::CompletionFailed.diagnostic()
            }
        }
    }
}

impl ErrorInfo for EspaceStateAccessError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Preparation { source } => source_diagnostic(source, Code::StateUnavailable),
            Self::Initialization { source } | Self::Operation { source, .. } => {
                source_diagnostic(source, Code::StateUnavailable)
            }
        }
    }
}

impl ErrorInfo for super::EspaceStateReadError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::StateAccess(error) => error.diagnostic(),
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::Poisoned => Code::StateUnavailable.diagnostic(),
            Self::ReadCallFailed { .. } => Code::Internal.diagnostic(),
        }
    }
}

impl ErrorInfo for EspaceContextError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Rpc(error) => error.diagnostic(),
            Self::StateAnchor(error) => error.diagnostic(),
            Self::EspaceBlockNotFound { .. } | Self::CoreSpacePivotNotFound { .. } => {
                Diagnostic::new(Code::ContextNotFound, self.to_string())
            }
            Self::CoreSpacePivotMismatch { .. } => {
                Diagnostic::new(Code::InconsistentContext, self.to_string())
            }
            Self::BlockContext(error) => error.diagnostic(),
        }
    }
}

impl ErrorInfo for EspaceExecutionError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::StateAccess(error) => error.diagnostic(),
            Self::ResultIntegration(_) => Code::ExecutionIntegrationFailed.diagnostic(),
        }
    }
}

impl ErrorInfo for EspaceSimulationError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Input(error) => error.diagnostic(),
            Self::Context(error) => error.diagnostic(),
            Self::Completion(error) => error.diagnostic(),
            Self::Execution(error) => error.diagnostic(),
            Self::Runtime(error) => error.diagnostic(),
        }
    }
}

impl ErrorInfo for super::EspaceAnalysisError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Validation { .. } => Code::AnalysisValidationFailed.diagnostic(),
            Self::Unsupported { .. } => Code::AnalysisUnsupported.diagnostic(),
            Self::StateRead(error) => error.diagnostic(),
            Self::Contract(error) => contract_diagnostic(error, |source| {
                source_diagnostic(source, Code::StateUnavailable)
            }),
            Self::Coverage(error) => error.diagnostic(),
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::RuleFailure { source, .. } => source_diagnostic(source.as_ref(), Code::Internal),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceAnalysisError {
    #[error("eSpace analysis validation failed: {details}")]
    Validation { details: String },
    #[error("unsupported eSpace behavior: {details}")]
    Unsupported { details: String },
    #[error(transparent)]
    StateRead(#[from] super::EspaceStateReadError),
    #[error(transparent)]
    Contract(#[from] contract_standards::analysis::AnalysisError),
    #[error(transparent)]
    Coverage(#[from] simulation_core::analysis::CoverageError),
    #[error(transparent)]
    LimitExceeded(#[from] AnalysisLimitExceeded),
    #[error("{rules} analysis failed: {source}")]
    RuleFailure {
        rules: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}
impl EspaceAnalysisError {
    pub fn rule_failure(
        rules: &'static str,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::RuleFailure {
            rules,
            source: Box::new(source),
        }
    }
}

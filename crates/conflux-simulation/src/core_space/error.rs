use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use conflux_provider::{CoreAddress, Network};
use simulation_core::error::{Diagnostic, DiagnosticData, ErrorCode as Code, ErrorInfo};
use simulation_core::observation::AnalysisLimitExceeded;
use std::error::Error as StdError;
use thiserror::Error;

use super::{CoreSpaceContextError, CoreSpaceTransactionInputError};
use crate::{
    ConfluxRpcError,
    error::{estimation_diagnostic, source_diagnostic},
};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceTransactionCompletionError {
    #[error("failed to estimate Core Space gas and storage collateral: {source}")]
    GasAndCollateralEstimation {
        #[source]
        source: ConfluxRpcError,
    },

    #[error(transparent)]
    Provider(#[from] ConfluxRpcError),

    #[error("estimated Core Space storage limit exceeds u64: {value}")]
    StorageLimitOutOfRange { value: U256 },

    #[error("calculated Core Space max fee per gas exceeds U256")]
    MaxFeePerGasOverflow,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceStateAccessError {
    #[error(transparent)]
    LimitExceeded(#[from] AnalysisLimitExceeded),
    #[error("Core Space state provider request failed: {source}")]
    Provider {
        #[source]
        source: ConfluxRpcError,
    },
    #[error("failed to prepare anchored Core Space state: {source}")]
    Preparation {
        #[source]
        source: StorageError,
    },
    #[error("failed to initialize anchored Core Space state: {source}")]
    Initialization {
        #[source]
        source: StateDbError,
    },
    #[error("Core Space state access failed during {operation}: {source}")]
    Operation {
        operation: &'static str,
        #[source]
        source: StateDbError,
    },
    #[error("failed to access {operation}: {source}")]
    RecordedState {
        operation: &'static str,
        #[source]
        source: StorageError,
    },
    #[error("Core Space state address uses network {actual}, expected {expected}")]
    AddressNetworkMismatch { expected: Network, actual: Network },
    #[error("Core Space read call failed: {details}")]
    ReadCall { details: String },
    #[error("Core Space state reader is unavailable after a read-call failure")]
    Unavailable,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceProtocolError {
    #[error("Core Space changes could not access required state during {operation}: {source}")]
    StateAccess {
        operation: &'static str,
        #[source]
        source: CoreSpaceStateAccessError,
    },
    #[error("Core Space execution is inconsistent with change analysis: {details}")]
    InconsistentExecution { details: String },
    #[error("Core Space change analysis does not support this operation: {details}")]
    UnsupportedOperation { details: String },
}

impl CoreSpaceProtocolError {
    pub(crate) fn state_access(operation: &'static str, source: CoreSpaceStateAccessError) -> Self {
        Self::StateAccess { operation, source }
    }

    pub(crate) fn inconsistent_execution(details: impl Into<String>) -> Self {
        Self::InconsistentExecution {
            details: details.into(),
        }
    }

    pub(crate) fn unsupported_operation(details: impl Into<String>) -> Self {
        Self::UnsupportedOperation {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceResultIntegrationError {
    #[error(
        "the Core Space executor returned inconsistent gas accounting: gas limit {gas_limit}, intrinsic gas {intrinsic_gas}, gas used {gas_used}, gas charged {gas_charged}"
    )]
    InvalidGasAccounting {
        gas_limit: U256,
        intrinsic_gas: u64,
        gas_used: u64,
        gas_charged: u64,
    },
    #[error(
        "successful Core Space contract creation did not report the expected address {address}"
    )]
    MissingCreatedContract { address: CoreAddress },
    #[error("failed to represent a Core Space address returned by execution: {details}")]
    InvalidCoreAddress { details: String },
    #[error("the Core Space executor returned an invalid or unsupported result: {details}")]
    InvalidExecutorOutput { details: String },
    #[error("executed Core Space transaction did not produce a committed execution trace")]
    MissingExecutionTrace,
    #[error("Core Space executor returned {field} value {value}, exceeding u64")]
    GasValueOutOfRange { field: &'static str, value: U256 },
}

impl CoreSpaceResultIntegrationError {
    pub(crate) fn invalid_executor_output(details: impl Into<String>) -> Self {
        Self::InvalidExecutorOutput {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceExecutionError {
    #[error(transparent)]
    StateAccess(#[from] CoreSpaceStateAccessError),
    #[error(transparent)]
    ResultIntegration(#[from] CoreSpaceResultIntegrationError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceSimulationError {
    #[error(transparent)]
    Input(#[from] CoreSpaceTransactionInputError),
    #[error(transparent)]
    Context(#[from] CoreSpaceContextError),
    #[error(transparent)]
    Completion(#[from] CoreSpaceTransactionCompletionError),
    #[error(transparent)]
    Execution(#[from] CoreSpaceExecutionError),
    #[error(transparent)]
    Runtime(#[from] simulation_core::simulation::RuntimeError),
}

impl ErrorInfo for CoreSpaceTransactionInputError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Fields(error) => error.diagnostic(),
            Self::IncompatibleField { field, .. } => {
                Diagnostic::new(Code::InvalidInput, self.to_string())
                    .with_data(DiagnosticData::Field { field })
            }
            Self::InvalidType { .. } | Self::AddressNetworkMismatch { .. } => {
                Diagnostic::new(Code::InvalidInput, self.to_string())
            }
        }
    }
}

impl ErrorInfo for CoreSpaceTransactionCompletionError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Provider(error) => error.diagnostic(),
            Self::GasAndCollateralEstimation { source } => estimation_diagnostic(source),
            Self::StorageLimitOutOfRange { .. } | Self::MaxFeePerGasOverflow => {
                Code::CompletionFailed.diagnostic()
            }
        }
    }
}

impl ErrorInfo for CoreSpaceStateAccessError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::Provider { source } => source.diagnostic(),
            Self::Preparation { source } | Self::RecordedState { source, .. } => {
                source_diagnostic(source, Code::StateUnavailable)
            }
            Self::Initialization { source } | Self::Operation { source, .. } => {
                source_diagnostic(source, Code::StateUnavailable)
            }
            Self::Unavailable => Code::StateUnavailable.diagnostic(),
            Self::AddressNetworkMismatch { .. } | Self::ReadCall { .. } => {
                Code::Internal.diagnostic()
            }
        }
    }
}

impl ErrorInfo for CoreSpaceProtocolError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::StateAccess { source, .. } => source.diagnostic(),
            Self::InconsistentExecution { .. } => Code::AnalysisValidationFailed.diagnostic(),
            Self::UnsupportedOperation { .. } => Code::AnalysisUnsupported.diagnostic(),
        }
    }
}

impl ErrorInfo for CoreSpaceContextError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Rpc(error) => error.diagnostic(),
            Self::StateAnchor(error) => error.diagnostic(),
            Self::PivotBlockNotFound { .. } | Self::EspaceBlockNotFound { .. } => {
                Diagnostic::new(Code::ContextNotFound, self.to_string())
            }
            Self::SelectedBlockIsNotPivot { .. } => {
                Diagnostic::new(Code::InconsistentContext, self.to_string())
            }
            Self::BlockContext(error) => error.diagnostic(),
            Self::ConsensusContextUnavailable { .. } => {
                Diagnostic::new(Code::ContextUnavailable, self.to_string())
            }
        }
    }
}

impl ErrorInfo for CoreSpaceExecutionError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::StateAccess(error) => error.diagnostic(),
            Self::ResultIntegration(_) => Code::ExecutionIntegrationFailed.diagnostic(),
        }
    }
}

impl ErrorInfo for CoreSpaceSimulationError {
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

impl ErrorInfo for super::CoreSpaceAnalysisError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Coverage(error) => error.diagnostic(),
            Self::Protocol(error) => error.diagnostic(),
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::RuleFailure { source, .. } => {
                source_diagnostic(source.as_ref(), Code::AnalysisValidationFailed)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoreSpaceAnalysisError {
    #[error(transparent)]
    Coverage(#[from] simulation_core::analysis::CoverageError),
    #[error(transparent)]
    LimitExceeded(#[from] AnalysisLimitExceeded),
    #[error(transparent)]
    Protocol(#[from] CoreSpaceProtocolError),
    #[error("Core Space change rule `{rules}` failed: {source}")]
    RuleFailure {
        rules: &'static str,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
    },
}

impl CoreSpaceAnalysisError {
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

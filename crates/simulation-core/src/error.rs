use crate::observation::{AnalysisLimitExceeded, AnalysisResource};
use alloy_primitives::U256;
#[cfg(feature = "serde")]
use serde::Serialize;

/// Stable error identities shared by library and service adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub enum ErrorCode {
    #[cfg_attr(feature = "serde", serde(rename = "input.invalid"))]
    InvalidInput,
    #[cfg_attr(feature = "serde", serde(rename = "context.not_found"))]
    ContextNotFound,
    #[cfg_attr(feature = "serde", serde(rename = "context.inconsistent"))]
    InconsistentContext,
    #[cfg_attr(feature = "serde", serde(rename = "context.unavailable"))]
    ContextUnavailable,
    #[cfg_attr(feature = "serde", serde(rename = "completion.failed"))]
    CompletionFailed,
    #[cfg_attr(feature = "serde", serde(rename = "simulation.unsupported"))]
    UnsupportedSimulation,
    #[cfg_attr(feature = "serde", serde(rename = "provider.request_failed"))]
    ProviderRequestFailed,
    #[cfg_attr(feature = "serde", serde(rename = "state.unavailable"))]
    StateUnavailable,
    #[cfg_attr(feature = "serde", serde(rename = "execution.integration_failed"))]
    ExecutionIntegrationFailed,
    #[cfg_attr(feature = "serde", serde(rename = "execution.engine_failed"))]
    ExecutionEngineFailed,
    #[cfg_attr(feature = "serde", serde(rename = "runtime.unavailable"))]
    RuntimeUnavailable,
    #[cfg_attr(feature = "serde", serde(rename = "internal.error"))]
    Internal,
    #[cfg_attr(feature = "serde", serde(rename = "transaction.rejected"))]
    TransactionRejected,
    #[cfg_attr(feature = "serde", serde(rename = "transaction.reverted"))]
    TransactionReverted,
    #[cfg_attr(feature = "serde", serde(rename = "transaction.halted"))]
    TransactionHalted,
    #[cfg_attr(feature = "serde", serde(rename = "analysis.unsupported"))]
    AnalysisUnsupported,
    #[cfg_attr(feature = "serde", serde(rename = "analysis.incomplete_evidence"))]
    IncompleteEvidence,
    #[cfg_attr(feature = "serde", serde(rename = "analysis.validation_failed"))]
    AnalysisValidationFailed,
    #[cfg_attr(feature = "serde", serde(rename = "analysis.limit_exceeded"))]
    AnalysisLimitExceeded,
    #[cfg_attr(feature = "serde", serde(rename = "service.closed"))]
    ServiceClosed,
    #[cfg_attr(feature = "serde", serde(rename = "service.timeout"))]
    ServiceTimeout,
    #[cfg_attr(feature = "serde", serde(rename = "service.cancelled"))]
    ServiceCancelled,
}

/// Safe public information about an error. Rust errors retain the diagnostic source.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub message: String,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub data: Option<DiagnosticData>,
}

/// Defined public details; backend errors remain in the Rust source chain.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(untagged, rename_all_fields = "camelCase"))]
pub enum DiagnosticData {
    Field {
        field: &'static str,
    },
    ConflictingFields {
        first: &'static str,
        second: &'static str,
    },
    ValueRange {
        field: &'static str,
        maximum: U256,
        actual: U256,
    },
    AnalysisLimit {
        resource: AnalysisResource,
        limit: usize,
    },
    ExecutionPosition {
        position: usize,
    },
    SolidityError {
        reason: String,
    },
    SolidityPanic {
        panic_code: U256,
    },
}

impl Diagnostic {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: DiagnosticData) -> Self {
        self.data = Some(data);
        self
    }
}

pub trait ErrorInfo {
    fn diagnostic(&self) -> Diagnostic;
}

impl ErrorInfo for Diagnostic {
    fn diagnostic(&self) -> Diagnostic {
        self.clone()
    }
}

impl ErrorInfo for crate::transaction::TransactionInputError {
    fn diagnostic(&self) -> Diagnostic {
        use crate::transaction::TransactionInputError::*;
        let data = match self {
            IncompatibleField { field, .. } | MissingField { field, .. } => {
                DiagnosticData::Field { field }
            }
            ConflictingFields { first, second } => {
                DiagnosticData::ConflictingFields { first, second }
            }
            OutOfRange {
                field,
                value,
                maximum,
            } => DiagnosticData::ValueRange {
                field,
                maximum: *maximum,
                actual: *value,
            },
            UnsupportedType { .. } => {
                return Diagnostic::new(ErrorCode::UnsupportedSimulation, self.to_string());
            }
        };
        Diagnostic::new(ErrorCode::InvalidInput, self.to_string()).with_data(data)
    }
}

impl ErrorInfo for AnalysisLimitExceeded {
    fn diagnostic(&self) -> Diagnostic {
        ErrorCode::AnalysisLimitExceeded
            .diagnostic()
            .with_data(DiagnosticData::AnalysisLimit {
                resource: self.resource,
                limit: self.limit,
            })
    }
}

impl ErrorInfo for contract_standards::SolidityRevertReason {
    fn diagnostic(&self) -> Diagnostic {
        let diagnostic = ErrorCode::TransactionReverted.diagnostic();
        let data = match self {
            Self::SolidityError { message } => DiagnosticData::SolidityError {
                reason: message.clone(),
            },
            Self::SolidityPanic { code } => DiagnosticData::SolidityPanic { panic_code: *code },
            _ => return diagnostic,
        };
        diagnostic.with_data(data)
    }
}

/// The backend supplies state-error classification without changing contract semantics.
pub fn contract_diagnostic(
    error: &contract_standards::analysis::AnalysisError,
    state_diagnostic: impl FnOnce(&(dyn std::error::Error + 'static)) -> Diagnostic,
) -> Diagnostic {
    use contract_standards::analysis::AnalysisError;
    match error {
        AnalysisError::State { source } => state_diagnostic(source.as_ref()),
        AnalysisError::Unsupported { .. } => ErrorCode::AnalysisUnsupported.diagnostic(),
        AnalysisError::IncompleteEvidence { .. } => ErrorCode::IncompleteEvidence.diagnostic(),
        AnalysisError::Validation { position, .. } => {
            let diagnostic = ErrorCode::AnalysisValidationFailed.diagnostic();
            match position {
                Some(position) => diagnostic.with_data(DiagnosticData::ExecutionPosition {
                    position: *position,
                }),
                None => diagnostic,
            }
        }
    }
}

impl ErrorCode {
    pub fn diagnostic(self) -> Diagnostic {
        Diagnostic::new(
            self,
            match self {
                Self::InvalidInput => "Invalid transaction input",
                Self::ContextNotFound => "Requested state context was not found",
                Self::InconsistentContext => "State context is inconsistent",
                Self::ContextUnavailable => "Required execution context is unavailable",
                Self::CompletionFailed => "Unable to complete the transaction",
                Self::UnsupportedSimulation => "Requested simulation capability is not supported",
                Self::ProviderRequestFailed => {
                    "Simulation service could not query the upstream chain node"
                }
                Self::StateUnavailable => "Required state is unavailable",
                Self::ExecutionIntegrationFailed => "Execution result could not be integrated",
                Self::ExecutionEngineFailed => "Execution engine failed",
                Self::RuntimeUnavailable => "Simulation runtime is unavailable",
                Self::Internal => "Internal error",
                Self::TransactionRejected => "Transaction was rejected",
                Self::TransactionReverted => "Transaction reverted",
                Self::TransactionHalted => "Transaction execution failed",
                Self::AnalysisUnsupported => "Unsupported contract behavior",
                Self::IncompleteEvidence => "Required analysis evidence is unavailable",
                Self::AnalysisValidationFailed => "Transaction changes could not be verified",
                Self::AnalysisLimitExceeded => "Analysis resource limit exceeded",
                Self::ServiceClosed => "Simulation service is closed",
                Self::ServiceTimeout => "Simulation response deadline exceeded",
                Self::ServiceCancelled => "Simulation task was cancelled",
            },
        )
    }
}

impl ErrorInfo for crate::simulation::RuntimeError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Unavailable => ErrorCode::RuntimeUnavailable,
            Self::Task(_) => ErrorCode::Internal,
        }
        .diagnostic()
    }
}

impl ErrorInfo for crate::analysis::CoverageError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Unsupported { .. } => ErrorCode::AnalysisUnsupported,
            Self::Incomplete { .. } => ErrorCode::IncompleteEvidence,
        }
        .diagnostic()
    }
}

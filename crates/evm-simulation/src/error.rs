use alloy::transports::TransportError;
use thiserror::Error;

use contract_standards::legacy::ContractStandardsError;

use crate::EvmBlockSelector;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmInitializationError {
    #[error("failed to fetch the Ethereum chain id: {source}")]
    ChainIdRequest {
        #[source]
        source: TransportError,
    },

    #[error("Ethereum chain id mismatch: expected {expected}, got {actual}")]
    ChainIdMismatch { expected: u64, actual: u64 },
}

impl EvmInitializationError {
    pub(crate) const fn chain_id_request(source: TransportError) -> Self {
        Self::ChainIdRequest { source }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum EvmContextError {
    #[error("failed to resolve block selected by {selector}: {source}")]
    Provider {
        selector: EvmBlockSelector,
        #[source]
        source: TransportError,
    },

    #[error("provider did not return the block selected by {selector}")]
    BlockNotFound { selector: EvmBlockSelector },
}

impl EvmContextError {
    pub(crate) const fn provider(selector: EvmBlockSelector, source: TransportError) -> Self {
        Self::Provider { selector, source }
    }
}

#[derive(Debug, Error)]
#[error("transaction completion failed: {details}")]
pub(crate) struct EvmTransactionCompletionError {
    details: String,
}

impl EvmTransactionCompletionError {
    pub(crate) fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmSimulationErrorKind {
    BlockResolution,
    TransactionCompletion,
    NotReady,
    BlockContext,
    StateAccess,
    Execution,
    ExecutionTask,
    Analysis,
    Unexpected,
}

impl EvmSimulationErrorKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::BlockResolution => "block_resolution_error",
            Self::TransactionCompletion => "transaction_resolution_error",
            Self::NotReady => "not_ready",
            Self::BlockContext => "block_context_error",
            Self::StateAccess => "state_access_error",
            Self::Execution => "simulation_execution_error",
            Self::ExecutionTask => "execution_task_error",
            Self::Analysis => "analysis_failed",
            Self::Unexpected => "unexpected",
        }
    }
}

#[derive(Debug, Error)]
pub enum EvmSimulationError {
    #[error("{0}")]
    NotSupported(String),

    #[error("{details}")]
    Internal {
        kind: EvmSimulationErrorKind,
        details: String,
    },
}

impl EvmSimulationError {
    pub fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }

    pub fn not_ready(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::NotReady, details)
    }

    pub(crate) fn block_resolution(error: EvmContextError) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::BlockResolution, error.to_string())
    }

    pub(crate) fn transaction_completion(error: EvmTransactionCompletionError) -> Self {
        Self::internal_kind(
            EvmSimulationErrorKind::TransactionCompletion,
            error.to_string(),
        )
    }

    pub fn block_context_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::BlockContext, details)
    }

    pub fn state_access_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::StateAccess, details)
    }

    pub fn execution_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::Execution, details)
    }

    pub(crate) fn execution_task_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::ExecutionTask, details)
    }

    pub fn analysis_failed(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::Analysis, details)
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationErrorKind::Unexpected, details)
    }

    pub const fn kind_code(&self) -> Option<&'static str> {
        match self {
            Self::NotSupported(_) => None,
            Self::Internal { kind, .. } => Some(kind.code()),
        }
    }

    pub const fn is_not_supported(&self) -> bool {
        matches!(self, Self::NotSupported(_))
    }

    pub fn details(&self) -> &str {
        match self {
            Self::NotSupported(details) | Self::Internal { details, .. } => details,
        }
    }

    fn internal_kind(kind: EvmSimulationErrorKind, details: impl Into<String>) -> Self {
        Self::Internal {
            kind,
            details: details.into(),
        }
    }
}

impl From<ContractStandardsError> for EvmSimulationError {
    fn from(error: ContractStandardsError) -> Self {
        Self::analysis_failed(format!("transaction changes failed: {error}"))
    }
}

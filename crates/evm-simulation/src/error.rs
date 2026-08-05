use thiserror::Error;

use contract_standards::ContractStandardsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmSimulationInternalKind {
    NotReady,
    BlockContext,
    StateAccess,
    Execution,
    Analysis,
    Unexpected,
}

impl EvmSimulationInternalKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotReady => "not_ready",
            Self::BlockContext => "block_context_error",
            Self::StateAccess => "state_access_error",
            Self::Execution => "engine_execution_error",
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
        kind: EvmSimulationInternalKind,
        details: String,
    },
}

impl EvmSimulationError {
    pub fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }

    pub fn not_ready(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::NotReady, details)
    }

    pub fn block_context_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::BlockContext, details)
    }

    pub fn state_access_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::StateAccess, details)
    }

    pub fn execution_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::Execution, details)
    }

    pub fn analysis_failed(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::Analysis, details)
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmSimulationInternalKind::Unexpected, details)
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

    fn internal_kind(kind: EvmSimulationInternalKind, details: impl Into<String>) -> Self {
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

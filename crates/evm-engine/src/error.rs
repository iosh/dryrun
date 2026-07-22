use thiserror::Error;

use crate::changes::TransactionChangesError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmEngineInternalKind {
    NotReady,
    BlockContext,
    StateAccess,
    Execution,
    Analysis,
    Unexpected,
}

impl EvmEngineInternalKind {
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
pub enum EvmEngineError {
    #[error("{0}")]
    NotSupported(String),

    #[error("{details}")]
    Internal {
        kind: EvmEngineInternalKind,
        details: String,
    },
}

impl EvmEngineError {
    pub fn not_supported(details: impl Into<String>) -> Self {
        Self::NotSupported(details.into())
    }

    pub fn not_ready(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::NotReady, details)
    }

    pub fn block_context_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::BlockContext, details)
    }

    pub fn state_access_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::StateAccess, details)
    }

    pub fn engine_execution_error(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::Execution, details)
    }

    pub fn analysis_failed(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::Analysis, details)
    }

    pub fn internal(details: impl Into<String>) -> Self {
        Self::internal_kind(EvmEngineInternalKind::Unexpected, details)
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

    fn internal_kind(kind: EvmEngineInternalKind, details: impl Into<String>) -> Self {
        Self::Internal {
            kind,
            details: details.into(),
        }
    }
}

impl From<TransactionChangesError> for EvmEngineError {
    fn from(error: TransactionChangesError) -> Self {
        Self::analysis_failed(format!("transaction changes failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{EvmEngineError, EvmEngineInternalKind};

    #[test]
    fn internal_kinds_expose_stable_codes() {
        let cases = [
            (EvmEngineInternalKind::NotReady, "not_ready"),
            (EvmEngineInternalKind::BlockContext, "block_context_error"),
            (EvmEngineInternalKind::StateAccess, "state_access_error"),
            (EvmEngineInternalKind::Execution, "engine_execution_error"),
            (EvmEngineInternalKind::Analysis, "analysis_failed"),
            (EvmEngineInternalKind::Unexpected, "unexpected"),
        ];

        for (kind, expected_code) in cases {
            assert_eq!(kind.code(), expected_code);
        }
    }

    #[test]
    fn generic_internal_error_uses_unexpected_kind() {
        let error = EvmEngineError::internal("unexpected engine state");

        assert!(matches!(
            error,
            EvmEngineError::Internal { kind, details }
                if kind == EvmEngineInternalKind::Unexpected
                    && kind.code() == "unexpected"
                    && details == "unexpected engine state"
        ));
    }

    #[test]
    fn error_accessors_expose_kind_and_details() {
        let internal = EvmEngineError::state_access_error("missing account state");
        assert!(!internal.is_not_supported());
        assert_eq!(internal.kind_code(), Some("state_access_error"));
        assert_eq!(internal.details(), "missing account state");

        let not_supported = EvmEngineError::not_supported("block.hash is not supported yet");
        assert!(not_supported.is_not_supported());
        assert_eq!(not_supported.kind_code(), None);
        assert_eq!(not_supported.details(), "block.hash is not supported yet");
    }
}

use thiserror::Error;

use crate::espace::{EspaceAnalysisError, EspaceExecutionPosition};

pub(super) fn state_mismatch_at(
    position: EspaceExecutionPosition,
    details: &'static str,
) -> EspaceAnalysisError {
    token_change_error_at(position, details)
}

pub(super) fn token_change_error(details: impl Into<String>) -> EspaceAnalysisError {
    EspaceAnalysisError::rule_failure(
        "token",
        TokenChangeError {
            details: details.into(),
        },
    )
}

pub(super) fn token_change_error_at(
    position: EspaceExecutionPosition,
    details: impl Into<String>,
) -> EspaceAnalysisError {
    token_change_error(format!(
        "at execution position {}: {}",
        position.index(),
        details.into()
    ))
}

#[derive(Debug, Error)]
#[error("{details}")]
struct TokenChangeError {
    details: String,
}

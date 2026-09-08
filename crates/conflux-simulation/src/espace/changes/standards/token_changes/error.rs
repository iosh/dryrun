use thiserror::Error;

use crate::espace::{EspaceChangesError, EspaceExecutionPosition};

pub(super) fn state_mismatch_at(
    position: EspaceExecutionPosition,
    details: &'static str,
) -> EspaceChangesError {
    token_change_error_at(position, details)
}

pub(super) fn token_change_error(details: impl Into<String>) -> EspaceChangesError {
    EspaceChangesError::derivation(
        "token",
        TokenChangeError {
            details: details.into(),
        },
    )
}

pub(super) fn token_change_error_at(
    position: EspaceExecutionPosition,
    details: impl Into<String>,
) -> EspaceChangesError {
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

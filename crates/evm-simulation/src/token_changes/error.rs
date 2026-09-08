use thiserror::Error;

use crate::{EvmChangeDerivationError, execution::EvmExecutionPosition};

pub(super) fn state_mismatch_at(
    position: EvmExecutionPosition,
    details: &'static str,
) -> EvmChangeDerivationError {
    token_change_error_at(position, details)
}

pub(super) fn token_change_error(details: impl Into<String>) -> EvmChangeDerivationError {
    EvmChangeDerivationError::rule_failure(
        "token",
        TokenChangeError::Details {
            details: details.into(),
        },
    )
}

pub(super) fn token_change_error_at(
    position: EvmExecutionPosition,
    details: impl Into<String>,
) -> EvmChangeDerivationError {
    token_change_error(format!(
        "at execution position {}: {}",
        position.index(),
        details.into()
    ))
}

#[derive(Debug, Error)]
enum TokenChangeError {
    #[error("{details}")]
    Details { details: String },
}

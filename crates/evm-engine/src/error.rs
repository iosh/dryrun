use thiserror::Error;

use contract_standards::{
    ContractStandardsError, StateArithmeticOperation, StatePhase, StateRequirement,
};

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

impl From<ContractStandardsError> for EvmEngineError {
    fn from(error: ContractStandardsError) -> Self {
        let details = contract_standards_error_details(&error);
        Self::analysis_failed(format!("transaction changes failed: {details}"))
    }
}

fn contract_standards_error_details(error: &ContractStandardsError) -> String {
    // Keep the existing EVM RPC details stable while the shared crate uses
    // chain-neutral terminology and a more precise record position.
    match error {
        ContractStandardsError::MalformedEvent { position, source } => format!(
            "failed to decode event at observation {}: {source}",
            position.index
        ),
        ContractStandardsError::StateValueMissing { requirement, phase } => {
            legacy_state_value_missing(*requirement, *phase)
        }
        ContractStandardsError::StateArithmetic {
            requirement,
            operation,
            current,
            amount,
        } => legacy_state_arithmetic(**requirement, *operation, *current, *amount),
        _ => error.to_string(),
    }
}

fn legacy_state_value_missing(requirement: StateRequirement, phase: StatePhase) -> String {
    match requirement {
        StateRequirement::TokenContractCode(address) => {
            format!("token state values are missing {phase} runtime code hash for {address}")
        }
        StateRequirement::CollectionStandards(collection) => {
            format!("token state values are missing {phase} collection standards for {collection}")
        }
        StateRequirement::Erc20Balance(key) => format!(
            "ERC-20 {phase} balance for {} in token {} is missing",
            key.account, key.token
        ),
        StateRequirement::Erc20TotalSupply(token) => {
            format!("ERC-20 {phase} total supply for token {token} is missing")
        }
        StateRequirement::Erc20Allowance(key) => format!(
            "ERC-20 {phase} allowance for owner {} and spender {} in token {} is missing",
            key.owner, key.spender, key.token
        ),
        StateRequirement::Erc721Token(key) => format!(
            "ERC-721 {phase} state for token {} in collection {} is missing",
            key.token_id, key.collection
        ),
        StateRequirement::Erc1155Balance(key) => format!(
            "ERC-1155 {phase} balance for {} and token {} in collection {} is missing",
            key.account, key.token_id, key.collection
        ),
        StateRequirement::OperatorApproval(key) => format!(
            "{phase} operator approval for owner {} and operator {} in collection {} is missing",
            key.owner, key.operator, key.collection
        ),
    }
}

fn legacy_state_arithmetic(
    requirement: StateRequirement,
    operation: StateArithmeticOperation,
    current: alloy_primitives::U256,
    amount: alloy_primitives::U256,
) -> String {
    match (requirement, operation) {
        (StateRequirement::Erc20Balance(key), StateArithmeticOperation::Subtract) => format!(
            "ERC-20 balance underflow for {} in token {}: balance {current}, cannot subtract {amount}",
            key.account, key.token
        ),
        (StateRequirement::Erc20Balance(key), StateArithmeticOperation::Add) => format!(
            "ERC-20 balance overflow for {} in token {}: balance {current}, cannot add {amount}",
            key.account, key.token
        ),
        (StateRequirement::Erc20TotalSupply(token), StateArithmeticOperation::Subtract) => format!(
            "ERC-20 total supply underflow for token {token}: total supply {current}, cannot subtract {amount}"
        ),
        (StateRequirement::Erc20TotalSupply(token), StateArithmeticOperation::Add) => format!(
            "ERC-20 total supply overflow for token {token}: total supply {current}, cannot add {amount}"
        ),
        (StateRequirement::Erc1155Balance(key), StateArithmeticOperation::Subtract) => format!(
            "ERC-1155 balance underflow for {} and token {} in collection {}: cannot subtract {amount}",
            key.account, key.token_id, key.collection
        ),
        (StateRequirement::Erc1155Balance(key), StateArithmeticOperation::Add) => format!(
            "ERC-1155 balance overflow for {} and token {} in collection {}: cannot add {amount}",
            key.account, key.token_id, key.collection
        ),
        (requirement, operation) => {
            format!(
                "state {operation} failed for {requirement}: current {current}, amount {amount}"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use contract_standards::{ContractStandardsError, StatePhase, StateRequirement};

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

    #[test]
    fn shared_state_errors_keep_existing_evm_details() {
        let address = Address::repeat_byte(0x11);
        let error = EvmEngineError::from(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::TokenContractCode(address),
            phase: StatePhase::Before,
        });

        assert_eq!(
            error.details(),
            format!(
                "transaction changes failed: token state values are missing \
                 before runtime code hash for {address}"
            )
        );
    }
}

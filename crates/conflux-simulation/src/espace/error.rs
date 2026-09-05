use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use contract_standards::MissingMetadataOutcome;
use thiserror::Error;
use tokio::task::JoinError;

use super::{EspaceContextError, EspaceTransactionInputError, TxType};
use crate::{ConfluxRpcError, ExecutionBlockContextError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceTransactionCompletionError {
    #[error(transparent)]
    Input(#[from] EspaceTransactionInputError),

    #[error("eSpace does not support {transaction_type} transactions")]
    UnsupportedTransactionType { transaction_type: TxType },

    #[error("failed to fetch the sender nonce at eSpace block {block_number}: {source}")]
    NonceLookup {
        block_number: u64,
        #[source]
        source: ConfluxRpcError,
    },
    #[error("sender nonce at eSpace block {block_number} exceeds u64: {value}")]
    NonceOutOfRange { block_number: u64, value: U256 },
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
    #[error("eSpace block {block_number} has no base fee for dynamic-fee completion")]
    MissingBaseFee { block_number: u64 },
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

    pub(crate) fn invalid_observed_fee_settlement(details: impl Into<String>) -> Self {
        Self::new(format!(
            "invalid observed fee settlement: {}",
            details.into()
        ))
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
    #[error("failed to construct the eSpace execution context: {source}")]
    Context {
        #[source]
        source: ExecutionBlockContextError,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceChangesError {
    #[error("eSpace execution is inconsistent with change analysis: {details}")]
    InconsistentExecution { details: String },
    #[error("{resolver} resolver could not produce complete eSpace changes: {source}")]
    Resolver {
        resolver: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[error("a decoded standard change is missing a required metadata outcome")]
    MissingMetadataOutcome {
        #[from]
        source: MissingMetadataOutcome,
    },
    #[error("eSpace state access failed: {details}")]
    StateAccess { details: String },
}

impl EspaceChangesError {
    pub(crate) fn inconsistent_execution(details: impl Into<String>) -> Self {
        Self::InconsistentExecution {
            details: details.into(),
        }
    }

    pub(crate) fn resolver(
        resolver: &'static str,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Resolver {
            resolver,
            source: Box::new(source),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceSimulationError {
    #[error(transparent)]
    Input(#[from] EspaceTransactionInputError),
    #[error(transparent)]
    Context(#[from] EspaceContextError),
    #[error(transparent)]
    Completion(EspaceTransactionCompletionError),
    #[error(transparent)]
    Execution(#[from] EspaceExecutionError),
    #[error(transparent)]
    Changes(#[from] EspaceChangesError),
    #[error("eSpace simulation requires an active Tokio runtime")]
    RuntimeUnavailable,
    #[error("blocking eSpace simulation task terminated unexpectedly: {source}")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },
}

impl From<EspaceTransactionCompletionError> for EspaceSimulationError {
    fn from(error: EspaceTransactionCompletionError) -> Self {
        match error {
            EspaceTransactionCompletionError::Input(input) => Self::Input(input),
            error => Self::Completion(error),
        }
    }
}

impl EspaceSimulationError {
    pub(crate) const fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }
}

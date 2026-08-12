use alloy_primitives::{Address, U256};
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use conflux_provider::AddressError;
use contract_standards::{MetadataCall, MissingMetadataOutcome};
use thiserror::Error;
use tokio::task::JoinError;

use super::{EspaceContextError, EspaceTransactionInputError};
use crate::{ConfluxRpcError, ExecutionBlockContextError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceTransactionCompletionError {
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
#[non_exhaustive]
pub enum EspaceResultIntegrationError {
    #[error(
        "the eSpace executor returned inconsistent gas accounting: gas limit {gas_limit}, intrinsic gas {intrinsic_gas}, gas used {gas_used}, gas charged {gas_charged}"
    )]
    InvalidGasAccounting {
        gas_limit: u64,
        intrinsic_gas: u64,
        gas_used: u64,
        gas_charged: u64,
    },
    #[error("burnt eSpace fee {burnt_amount} exceeds charged fee {charged_amount}")]
    BurntFeeExceedsCharged {
        charged_amount: U256,
        burnt_amount: U256,
    },
    #[error("invalid observed eSpace fee settlement: {details}")]
    InvalidObservedFeeSettlement { details: String },
    #[error("successful eSpace contract creation did not report the expected address {address}")]
    MissingCreatedContract { address: Address },
    #[error("failed to represent a committed Conflux log address: {source}")]
    InvalidLogAddress {
        #[source]
        source: AddressError,
    },
    #[error("the eSpace executor returned an invalid or unsupported result: {details}")]
    InvalidExecutorOutput { details: String },
    #[error("executed eSpace transaction did not produce a committed execution trace")]
    MissingExecutionTrace,
    #[error("eSpace executor returned {field} value {value}, exceeding u64")]
    GasValueOutOfRange { field: &'static str, value: U256 },
}

impl EspaceResultIntegrationError {
    pub(crate) fn invalid_observed_fee_settlement(details: impl Into<String>) -> Self {
        Self::InvalidObservedFeeSettlement {
            details: details.into(),
        }
    }

    pub(crate) fn invalid_executor_output(details: impl Into<String>) -> Self {
        Self::InvalidExecutorOutput {
            details: details.into(),
        }
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
pub enum EspaceNativeChangeError {
    #[error("native balance for eSpace account {address} is unavailable")]
    BalanceMissing { address: Address },
    #[error(
        "native balance underflow for eSpace account {address}: balance {balance}, debit {amount}"
    )]
    BalanceUnderflow {
        address: Address,
        balance: U256,
        amount: U256,
    },
    #[error(
        "native balance overflow for eSpace account {address}: balance {balance}, credit {amount}"
    )]
    BalanceOverflow {
        address: Address,
        balance: U256,
        amount: U256,
    },
    #[error(
        "native balance mismatch for eSpace account {address}: replayed {replayed}, state {actual}"
    )]
    BalanceMismatch {
        address: Address,
        replayed: U256,
        actual: U256,
    },
    #[error("unsupported cross-space native movement in an eSpace transaction: {details}")]
    UnsupportedCrossSpaceMovement { details: String },
    #[error("unsupported eSpace native balance operation: {details}")]
    UnsupportedBalanceOperation { details: String },
    #[error("failed eSpace execution retained a committed business native effect")]
    BusinessEffectOnFailedExecution,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceChangesError {
    #[error("eSpace change analysis failed to read state during {operation}: {source}")]
    StateRead {
        operation: &'static str,
        #[source]
        source: StateDbError,
    },
    #[error(transparent)]
    Native(#[from] EspaceNativeChangeError),
    #[error("eSpace execution is inconsistent with change analysis: {details}")]
    InconsistentExecution { details: String },
    #[error("metadata probe {call:?} failed to access anchored state: {source}")]
    MetadataStateAccess {
        call: MetadataCall<Address>,
        #[source]
        source: StateDbError,
    },
    #[error("metadata probe {call:?} could not be isolated: {details}")]
    MetadataProbeExecution {
        call: MetadataCall<Address>,
        details: String,
    },
    #[error("a decoded standard change is missing a required metadata outcome")]
    MissingMetadataOutcome {
        #[from]
        source: MissingMetadataOutcome,
    },
}

impl EspaceChangesError {
    pub(crate) fn inconsistent_execution(details: impl Into<String>) -> Self {
        Self::InconsistentExecution {
            details: details.into(),
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
    Completion(#[from] EspaceTransactionCompletionError),
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

impl EspaceSimulationError {
    pub(crate) const fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }
}

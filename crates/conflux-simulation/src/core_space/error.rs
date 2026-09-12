use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use conflux_provider::{CoreAddress, Network};
use thiserror::Error;
use tokio::task::JoinError;

use super::{
    CoreSpaceContextError, CoreSpaceTransactionCompletionError, CoreSpaceTransactionInputError,
};
use crate::{ConfluxRpcError, execution::ExecutionBlockContextError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceStateAccessError {
    #[error("Core Space state provider request failed: {source}")]
    Provider {
        #[source]
        source: ConfluxRpcError,
    },
    #[error("failed to prepare anchored Core Space state: {source}")]
    Preparation {
        #[source]
        source: StorageError,
    },
    #[error("failed to initialize anchored Core Space state: {source}")]
    Initialization {
        #[source]
        source: StateDbError,
    },
    #[error("Core Space state access failed during {operation}: {source}")]
    Operation {
        operation: &'static str,
        #[source]
        source: StateDbError,
    },
    #[error("failed to access {operation}: {source}")]
    RecordedState {
        operation: &'static str,
        #[source]
        source: StorageError,
    },
    #[error("Core Space state address uses network {actual}, expected {expected}")]
    AddressNetworkMismatch { expected: Network, actual: Network },
    #[error("Core Space read call failed: {details}")]
    ReadCall { details: String },
    #[error("Core Space state reader is unavailable after a read-call failure")]
    Unavailable,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceChangesError {
    #[error("Core Space changes could not access required state during {operation}: {source}")]
    StateAccess {
        operation: &'static str,
        #[source]
        source: CoreSpaceStateAccessError,
    },
    #[error("Core Space execution is inconsistent with change analysis: {details}")]
    InconsistentExecution { details: String },
    #[error("Core Space change analysis does not support this operation: {details}")]
    UnsupportedOperation { details: String },
}

impl CoreSpaceChangesError {
    pub(crate) fn state_access(operation: &'static str, source: CoreSpaceStateAccessError) -> Self {
        Self::StateAccess { operation, source }
    }

    pub(crate) fn inconsistent_execution(details: impl Into<String>) -> Self {
        Self::InconsistentExecution {
            details: details.into(),
        }
    }

    pub(crate) fn unsupported_operation(details: impl Into<String>) -> Self {
        Self::UnsupportedOperation {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceResultIntegrationError {
    #[error(
        "the Core Space executor returned inconsistent gas accounting: gas limit {gas_limit}, intrinsic gas {intrinsic_gas}, gas used {gas_used}, gas charged {gas_charged}"
    )]
    InvalidGasAccounting {
        gas_limit: U256,
        intrinsic_gas: u64,
        gas_used: u64,
        gas_charged: u64,
    },
    #[error(
        "successful Core Space contract creation did not report the expected address {address}"
    )]
    MissingCreatedContract { address: CoreAddress },
    #[error("failed to represent a Core Space address returned by execution: {details}")]
    InvalidCoreAddress { details: String },
    #[error("the Core Space executor returned an invalid or unsupported result: {details}")]
    InvalidExecutorOutput { details: String },
    #[error("executed Core Space transaction did not produce a committed execution trace")]
    MissingExecutionTrace,
    #[error("Core Space executor returned {field} value {value}, exceeding u64")]
    GasValueOutOfRange { field: &'static str, value: U256 },
}

impl CoreSpaceResultIntegrationError {
    pub(crate) fn invalid_executor_output(details: impl Into<String>) -> Self {
        Self::InvalidExecutorOutput {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceExecutionError {
    #[error(transparent)]
    StateAccess(#[from] CoreSpaceStateAccessError),
    #[error(transparent)]
    ResultIntegration(#[from] CoreSpaceResultIntegrationError),
    #[error("failed to construct the Core Space execution context: {source}")]
    Context {
        #[source]
        source: ExecutionBlockContextError,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceSimulationError {
    #[error(transparent)]
    Input(#[from] CoreSpaceTransactionInputError),
    #[error(transparent)]
    Context(#[from] CoreSpaceContextError),
    #[error(transparent)]
    Completion(#[from] CoreSpaceTransactionCompletionError),
    #[error(transparent)]
    Execution(#[from] CoreSpaceExecutionError),
    #[error(transparent)]
    Changes(#[from] CoreSpaceChangesError),
    #[error("Core Space simulation requires an active Tokio runtime")]
    RuntimeUnavailable,
    #[error("blocking Core Space simulation task terminated unexpectedly: {source}")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },
}

impl CoreSpaceSimulationError {
    pub(crate) const fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }
}

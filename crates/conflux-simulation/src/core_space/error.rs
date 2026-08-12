use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use cfx_storage::Error as StorageError;
use conflux_provider::CoreAddress;
use thiserror::Error;
use tokio::task::JoinError;

use super::{
    CoreSpaceContextError, CoreSpaceTransactionCompletionError, CoreSpaceTransactionInputError,
};
use crate::{
    ConfluxRpcError, ConfluxSimulationError,
    execution::{ExecutionBlockContextError, TransactionExecutionError},
};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceStateAccessError {
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
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceStorageSponsorshipError {
    #[error("failed to inspect Core Space contract {contract} for storage sponsorship: {source}")]
    ContractCodeLookup {
        contract: CoreAddress,
        #[source]
        source: ConfluxRpcError,
    },
    #[error(
        "failed to resolve storage sponsorship for sender {sender} and contract {contract}: {source}"
    )]
    EligibilityLookup {
        sender: CoreAddress,
        contract: CoreAddress,
        #[source]
        source: ConfluxRpcError,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceChangesError {
    #[error("Core Space change analysis could not access anchored state: {details}")]
    StateAccess { details: String },
    #[error("Core Space execution facts were inconsistent: {details}")]
    InvalidExecutionFacts { details: String },
    #[error("Core Space change analysis failed: {details}")]
    Analysis { details: String },
}

impl CoreSpaceChangesError {
    pub(crate) fn from_internal(error: ConfluxSimulationError) -> Self {
        match error {
            ConfluxSimulationError::StateAccess { message } => {
                Self::StateAccess { details: message }
            }
            ConfluxSimulationError::ExecutionInternal { message } => {
                Self::InvalidExecutionFacts { details: message }
            }
            ConfluxSimulationError::Analysis { message } => Self::Analysis { details: message },
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
    #[error("failed to construct the Core Space execution context: {source}")]
    Context {
        #[source]
        source: ExecutionBlockContextError,
    },
    #[error(transparent)]
    StateAccess(#[from] CoreSpaceStateAccessError),
    #[error(transparent)]
    Integration(#[from] CoreSpaceResultIntegrationError),
}

impl From<TransactionExecutionError> for CoreSpaceExecutionError {
    fn from(error: TransactionExecutionError) -> Self {
        match error {
            TransactionExecutionError::BlockContext(source) => Self::Context { source },
            TransactionExecutionError::StateAccess(source) => {
                Self::StateAccess(CoreSpaceStateAccessError::Operation {
                    operation: "execute Core Space transaction",
                    source,
                })
            }
            error => Self::Integration(CoreSpaceResultIntegrationError::invalid_executor_output(
                error.to_string(),
            )),
        }
    }
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
    StorageSponsorship(#[from] CoreSpaceStorageSponsorshipError),
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

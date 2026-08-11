use alloy_primitives::U256;
use cfx_statedb::Error as StateDbError;
use conflux_provider::CoreAddress;
use thiserror::Error;

use crate::execution::{ExecutionBlockContextError, TransactionExecutionError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceStateAccessError {
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

use alloy_primitives::{Bytes, U256 as AlloyU256};
use cfx_executor::executive::{
    Executed, ExecutionError, ExecutionOutcome as UpstreamExecutionOutcome, ToRepackError,
    TxDropError,
};
use cfx_statedb::Error as StateDbError;
use cfx_types::{AddressWithSpace, U256};
use primitives::{LogEntry, receipt::StorageChange};
use thiserror::Error;

use super::{
    ExecutionBlockContextError,
    observer::{Observation, ObservationKey},
};
use crate::primitive::u256_from_cfx;

/// Conflux execution output owned by this crate rather than by the upstream type map.
#[derive(Debug)]
pub(crate) struct ConfluxExecutionDetails {
    pub(crate) gas_used: u64,
    pub(crate) gas_charged: u64,
    pub(crate) fee: AlloyU256,
    pub(crate) burnt_fee: Option<AlloyU256>,
    pub(crate) output: Bytes,
}

#[derive(Debug)]
#[expect(
    dead_code,
    reason = "some details are carried for the next analysis slices before they have consumers"
)]
pub(crate) struct ConfluxExecutionOutput {
    pub(crate) common: ConfluxExecutionDetails,
    pub(crate) base_gas: u64,
    pub(crate) gas_sponsor_paid: bool,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) storage_sponsor_paid: bool,
    pub(crate) storage_collateralized: Vec<StorageChange>,
    pub(crate) storage_released: Vec<StorageChange>,
    pub(crate) contracts_created: Vec<AddressWithSpace>,
    pub(crate) observations: Vec<Observation>,
}

/// The normalized outcome exchanged between shared execution and each space.
#[derive(Debug)]
pub(crate) enum ConfluxExecutionOutcome {
    Success(ConfluxExecutionOutput),
    Failed {
        error: ExecutionError,
        details: ConfluxExecutionOutput,
    },
    NotExecutedDrop(TxDropError),
    NotExecutedToReconsiderPacking(ToRepackError),
}

#[derive(Debug, Error)]
pub(crate) enum TransactionExecutionError {
    #[error("execution block context failed: {0}")]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("state access failed: {0}")]
    StateAccess(#[source] StateDbError),

    #[error("executed transaction did not produce a valid observation journal")]
    MissingObservations,

    #[error(
        "execution returned {field} value {value}, exceeding the simulator maximum \
         18446744073709551615"
    )]
    GasValueOutOfRange { field: &'static str, value: U256 },
}

impl From<StateDbError> for TransactionExecutionError {
    fn from(error: StateDbError) -> Self {
        Self::StateAccess(error)
    }
}

impl ConfluxExecutionOutcome {
    pub(crate) fn from_upstream(
        outcome: UpstreamExecutionOutcome,
    ) -> Result<Self, TransactionExecutionError> {
        Ok(match outcome {
            UpstreamExecutionOutcome::Finished(executed) => {
                Self::Success(executed_transaction_details(executed)?)
            }
            UpstreamExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => Self::Failed {
                error,
                details: executed_transaction_details(executed)?,
            },
            UpstreamExecutionOutcome::NotExecutedDrop(error) => Self::NotExecutedDrop(error),
            UpstreamExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
                Self::NotExecutedToReconsiderPacking(error)
            }
        })
    }
}

fn executed_transaction_details(
    executed: Executed,
) -> Result<ConfluxExecutionOutput, TransactionExecutionError> {
    let Executed {
        base_gas,
        gas_used,
        fee,
        burnt_fee,
        gas_charged,
        gas_sponsor_paid,
        logs,
        storage_sponsor_paid,
        storage_collateralized,
        storage_released,
        contracts_created,
        output,
        mut ext_result,
    } = executed;

    let observations = ext_result
        .remove::<ObservationKey>()
        .ok_or(TransactionExecutionError::MissingObservations)?;
    let gas_used =
        u64::try_from(gas_used).map_err(|_| TransactionExecutionError::GasValueOutOfRange {
            field: "gas used",
            value: gas_used,
        })?;
    let gas_charged =
        u64::try_from(gas_charged).map_err(|_| TransactionExecutionError::GasValueOutOfRange {
            field: "gas charged",
            value: gas_charged,
        })?;

    Ok(ConfluxExecutionOutput {
        common: ConfluxExecutionDetails {
            gas_used,
            gas_charged,
            fee: u256_from_cfx(fee),
            burnt_fee: burnt_fee.map(u256_from_cfx),
            output: Bytes::from(output),
        },
        base_gas,
        gas_sponsor_paid,
        logs,
        storage_sponsor_paid,
        storage_collateralized,
        storage_released,
        contracts_created,
        observations,
    })
}

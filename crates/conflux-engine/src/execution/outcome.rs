use cfx_bytes::Bytes;
use cfx_executor::executive::{
    Executed, ExecutionError, ExecutionOutcome as UpstreamExecutionOutcome, ToRepackError,
    TxDropError,
};
use cfx_statedb::Error as StateDbError;
use cfx_types::{AddressWithSpace, U256};
use primitives::{LogEntry, receipt::StorageChange};
use thiserror::Error;

use super::observer::{Observation, ObservationKey};

/// Execution details owned by this crate rather than by the upstream type map.
#[derive(Debug)]
#[expect(
    dead_code,
    reason = "some details are carried for the next analysis slices before they have consumers"
)]
pub(crate) struct ExecutedTransactionDetails {
    pub(crate) base_gas: u64,
    pub(crate) gas_used: U256,
    pub(crate) fee: U256,
    pub(crate) burnt_fee: Option<U256>,
    pub(crate) gas_charged: U256,
    pub(crate) gas_sponsor_paid: bool,
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) storage_sponsor_paid: bool,
    pub(crate) storage_collateralized: Vec<StorageChange>,
    pub(crate) storage_released: Vec<StorageChange>,
    pub(crate) contracts_created: Vec<AddressWithSpace>,
    pub(crate) output: Bytes,
    pub(crate) observations: Vec<Observation>,
}

/// The normalized outcome exchanged between shared execution and each space.
#[derive(Debug)]
pub(crate) enum TransactionExecutionOutcome {
    Success(ExecutedTransactionDetails),
    Failed {
        error: ExecutionError,
        details: ExecutedTransactionDetails,
    },
    NotExecutedDrop(TxDropError),
    NotExecutedToReconsiderPacking(ToRepackError),
}

#[derive(Debug, Error)]
pub(crate) enum TransactionExecutionError {
    #[error("state access failed: {0}")]
    StateAccess(#[source] StateDbError),

    #[error("executed transaction did not produce a valid observation journal")]
    MissingObservations,
}

impl From<StateDbError> for TransactionExecutionError {
    fn from(error: StateDbError) -> Self {
        Self::StateAccess(error)
    }
}

impl TransactionExecutionOutcome {
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

    pub(crate) fn into_executed(self) -> Option<ExecutedTransactionDetails> {
        match self {
            Self::Success(details) | Self::Failed { details, .. } => Some(details),
            Self::NotExecutedDrop(_) | Self::NotExecutedToReconsiderPacking(_) => None,
        }
    }
}

fn executed_transaction_details(
    executed: Executed,
) -> Result<ExecutedTransactionDetails, TransactionExecutionError> {
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

    Ok(ExecutedTransactionDetails {
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
        observations,
    })
}

#[cfg(test)]
mod tests {
    use cfx_executor::executive::{Executed, ExecutionOutcome as UpstreamExecutionOutcome};
    use cfx_types::U256;
    use typemap::ShareDebugMap;

    use super::{TransactionExecutionError, TransactionExecutionOutcome};
    use crate::execution::observer::ObservationKey;

    #[test]
    fn executed_outcome_distinguishes_missing_and_empty_observations() {
        let missing = TransactionExecutionOutcome::from_upstream(
            UpstreamExecutionOutcome::Finished(executed(ShareDebugMap::custom())),
        );
        assert!(matches!(
            missing,
            Err(TransactionExecutionError::MissingObservations)
        ));

        let mut ext_result = ShareDebugMap::custom();
        ext_result.insert::<ObservationKey>(Vec::new());
        let present = TransactionExecutionOutcome::from_upstream(
            UpstreamExecutionOutcome::Finished(executed(ext_result)),
        )
        .expect("empty observation journal is valid");

        let TransactionExecutionOutcome::Success(details) = present else {
            panic!("finished upstream outcome should remain successful");
        };
        assert!(details.observations.is_empty());
    }

    fn executed(ext_result: ShareDebugMap) -> Executed {
        Executed {
            base_gas: 0,
            gas_used: U256::zero(),
            fee: U256::zero(),
            burnt_fee: None,
            gas_charged: U256::zero(),
            gas_sponsor_paid: false,
            logs: Vec::new(),
            storage_sponsor_paid: false,
            storage_collateralized: Vec::new(),
            storage_released: Vec::new(),
            contracts_created: Vec::new(),
            output: Vec::new(),
            ext_result,
        }
    }
}

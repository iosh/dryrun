mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};
use thiserror::Error;

use super::{ChangeOccurrence, EspaceNativeCurrency};
use crate::espace::{EspaceChangesError, EspaceExecutedTransaction, EspaceStateAccess};

#[derive(Debug, Error)]
#[error("{details}")]
pub(super) struct NativeResolverDiagnostic {
    details: String,
}

impl NativeResolverDiagnostic {
    pub(super) fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }
}

pub(super) fn from_execution(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    currency: &EspaceNativeCurrency,
) -> Result<Vec<ChangeOccurrence>, EspaceChangesError> {
    let operations = collection::collect_native_operations(execution, state.written_accounts())
        .map_err(|error| EspaceChangesError::resolver("native currency", error))?;
    let before_balances = verification::read_native_balances(
        state.initial(),
        "read pre-execution native balances",
        &operations,
    )?;
    let after_balances = verification::read_native_balances(
        state.finalized(),
        "read post-execution native balances",
        &operations,
    )?;

    verification::verify_native_changes(&operations, &before_balances, &after_balances, currency)
        .map_err(|error| EspaceChangesError::resolver("native currency", error))
}

#[derive(Debug)]
struct NativeOperations {
    balance_accounts: Vec<Address>,
    operations: Vec<NativeOperation>,
}

impl NativeOperations {
    fn from_operations(mut operations: Vec<NativeOperation>, written_accounts: &[Address]) -> Self {
        operations.sort_by_key(NativeOperation::position);

        let mut balance_accounts: BTreeSet<_> = written_accounts.iter().copied().collect();
        for operation in &operations {
            match operation {
                NativeOperation::AccountTransfer { from, to, .. } => {
                    balance_accounts.insert(*from);
                    balance_accounts.insert(*to);
                }
                NativeOperation::SelfDestructBurn { contract, .. } => {
                    balance_accounts.insert(*contract);
                }
                NativeOperation::GasPrecharge { payer, .. } => {
                    balance_accounts.insert(*payer);
                }
                NativeOperation::GasRefund { recipient, .. } => {
                    balance_accounts.insert(*recipient);
                }
            }
        }

        Self {
            balance_accounts: balance_accounts.into_iter().collect(),
            operations,
        }
    }
}

#[derive(Debug)]
enum NativeOperation {
    AccountTransfer {
        position: usize,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: usize,
        contract: Address,
        amount: U256,
    },
    GasPrecharge {
        position: usize,
        payer: Address,
        amount: U256,
    },
    GasRefund {
        position: usize,
        recipient: Address,
        amount: U256,
    },
}

impl NativeOperation {
    fn position(&self) -> usize {
        match self {
            Self::AccountTransfer { position, .. }
            | Self::SelfDestructBurn { position, .. }
            | Self::GasPrecharge { position, .. }
            | Self::GasRefund { position, .. } => *position,
        }
    }
}

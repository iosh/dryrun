mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};
use thiserror::Error;

use super::{ChangeOccurrence, EspaceNativeCurrency};
use crate::espace::{EspaceChangesError, EspaceExecutedTransaction};

pub(super) use verification::NativeBalances;

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

pub(super) struct NativeAnalysis {
    operations: NativeOperations,
}

impl NativeAnalysis {
    pub(super) fn from_execution(
        execution: &EspaceExecutedTransaction,
    ) -> Result<Self, NativeResolverDiagnostic> {
        Ok(Self {
            operations: collection::collect_native_operations(execution)?,
        })
    }

    pub(super) fn read_balances(
        &self,
        state: &crate::espace::EspaceStateReader,
        operation: &'static str,
    ) -> Result<NativeBalances, EspaceChangesError> {
        verification::read_native_balances(state, operation, &self.operations)
    }

    pub(super) fn verify(
        &self,
        before_balances: &NativeBalances,
        after_balances: &NativeBalances,
        currency: &EspaceNativeCurrency,
    ) -> Result<Vec<ChangeOccurrence>, NativeResolverDiagnostic> {
        verification::verify_native_changes(
            &self.operations,
            before_balances,
            after_balances,
            currency,
        )
    }
}

#[derive(Debug)]
struct NativeOperations {
    balance_accounts: Vec<Address>,
    operations: Vec<NativeOperation>,
}

impl NativeOperations {
    fn from_operations(mut operations: Vec<NativeOperation>) -> Self {
        operations.sort_by_key(NativeOperation::position);

        let mut balance_accounts = BTreeSet::new();
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

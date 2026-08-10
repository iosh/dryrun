mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};

use crate::execution::ConfluxExecutionOutput;

use super::{ChangeOccurrence, EspaceNativeCurrency};
use crate::espace::{EspaceChangesError, EspaceNativeChangeError};

pub(super) use verification::NativeBalances;

pub(super) struct NativeAnalysis {
    operations: NativeOperations,
}

impl NativeAnalysis {
    pub(super) fn from_execution(
        execution: &ConfluxExecutionOutput,
    ) -> Result<Self, EspaceNativeChangeError> {
        Ok(Self {
            operations: collection::collect_native_operations(&execution.observations)?,
        })
    }

    pub(super) fn read_balances(
        &self,
        state: &cfx_executor::state::State,
        operation: &'static str,
    ) -> Result<NativeBalances, EspaceChangesError> {
        verification::read_native_balances(state, operation, &self.operations)
    }

    pub(super) fn verify(
        &self,
        before_balances: &NativeBalances,
        after_balances: &NativeBalances,
        successful: bool,
        currency: &EspaceNativeCurrency,
    ) -> Result<Vec<ChangeOccurrence>, EspaceNativeChangeError> {
        verification::verify_native_changes(
            &self.operations,
            before_balances,
            after_balances,
            successful,
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
    fn from_operations(operations: Vec<NativeOperation>) -> Self {
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
        payer: Address,
        amount: U256,
    },
    GasRefund {
        recipient: Address,
        amount: U256,
    },
}

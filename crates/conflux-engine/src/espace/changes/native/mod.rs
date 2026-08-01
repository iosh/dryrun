mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};
use contract_standards::Position;

pub(crate) use collection::collect_native_operations;
pub(crate) use verification::{read_native_balances, verify_native_changes};

#[derive(Debug)]
pub(crate) struct NativeOperations {
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
        position: Position,
        from: Address,
        to: Address,
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

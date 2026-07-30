mod collection;
mod verification;

use std::collections::BTreeSet;

use alloy_primitives::{Address, U256};
use contract_standards::Position;

pub(crate) use collection::collect_native_evidence;
pub(crate) use verification::{read_native_balances, verify_native_changes};

#[derive(Debug)]
pub(crate) struct NativeEvidence {
    accounts: Vec<Address>,
    operations: Vec<NativeOperation>,
}

impl NativeEvidence {
    fn from_operations(operations: Vec<NativeOperation>) -> Self {
        let mut accounts = BTreeSet::new();

        for operation in &operations {
            match operation {
                NativeOperation::Transfer { from, to, .. } => {
                    accounts.insert(*from);
                    accounts.insert(*to);
                }
                NativeOperation::GasPrecharge { payer, .. } => {
                    accounts.insert(*payer);
                }
                NativeOperation::GasRefund { recipient, .. } => {
                    accounts.insert(*recipient);
                }
            }
        }

        Self {
            accounts: accounts.into_iter().collect(),
            operations,
        }
    }
}

#[derive(Debug)]
enum NativeOperation {
    Transfer {
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

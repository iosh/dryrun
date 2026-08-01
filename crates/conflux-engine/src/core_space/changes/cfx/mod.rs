mod collection;
mod verification;

use std::{collections::BTreeSet, fmt};

use alloy_primitives::{Address, U256};
use contract_standards::Position;
use primitives::{Action, SignedTransaction};

pub(crate) use collection::collect_cfx_operations;
pub(crate) use verification::{read_cfx_state_snapshot, verify_cfx_changes};

use crate::{ConfluxEngineError, primitive::address_from_cfx};

#[derive(Debug)]
pub(crate) struct CfxOperations {
    balance_locations: Vec<CfxBalanceLocation>,
    operations: Vec<CfxOperation>,
}

impl CfxOperations {
    fn from_operations(operations: Vec<CfxOperation>) -> Self {
        let mut balance_locations = BTreeSet::new();

        for operation in &operations {
            match operation {
                CfxOperation::AccountTransfer { from, to, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *from });
                    balance_locations.insert(CfxBalanceLocation::Account { account: *to });
                }
                CfxOperation::GasPrecharge { payer, .. } => {
                    balance_locations.insert(*payer);
                }
                CfxOperation::GasRefund { recipient, .. } => {
                    balance_locations.insert(*recipient);
                }
                CfxOperation::StakingDeposit { account, .. }
                | CfxOperation::StakingWithdrawal { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *account });
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
                CfxOperation::NativeBurn { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Account { account: *account });
                }
                CfxOperation::StakingBurn { account, .. } => {
                    balance_locations.insert(CfxBalanceLocation::Staking { account: *account });
                }
            }
        }

        Self {
            balance_locations: balance_locations.into_iter().collect(),
            operations,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CfxBalanceLocation {
    Account { account: Address },
    Staking { account: Address },
    GasSponsor { contract_address: Address },
}

impl fmt::Display for CfxBalanceLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account { account } => {
                write!(formatter, "balance for account {account}")
            }
            Self::Staking { account } => {
                write!(formatter, "staking balance for account {account}")
            }
            Self::GasSponsor { contract_address } => {
                write!(
                    formatter,
                    "gas sponsor balance for contract {contract_address}"
                )
            }
        }
    }
}

#[derive(Debug)]
enum CfxOperation {
    AccountTransfer {
        position: Position,
        from: Address,
        to: Address,
        amount: U256,
    },
    GasPrecharge {
        payer: CfxBalanceLocation,
        amount: U256,
    },
    GasRefund {
        recipient: CfxBalanceLocation,
        amount: U256,
    },
    StakingDeposit {
        position: Position,
        account: Address,
        amount: U256,
    },
    StakingWithdrawal {
        position: Position,
        account: Address,
        principal_amount: U256,
        reward_amount: U256,
    },
    NativeBurn {
        position: Position,
        account: Address,
        amount: U256,
    },
    StakingBurn {
        position: Position,
        account: Address,
        amount: U256,
    },
}

pub(crate) fn determine_gas_fee_payer(
    transaction: &SignedTransaction,
    gas_paid_by_sponsor: bool,
) -> Result<CfxBalanceLocation, ConfluxEngineError> {
    if !gas_paid_by_sponsor {
        return Ok(CfxBalanceLocation::Account {
            account: address_from_cfx(transaction.sender().address),
        });
    }

    match transaction.action() {
        Action::Call(contract_address) => Ok(CfxBalanceLocation::GasSponsor {
            contract_address: address_from_cfx(contract_address),
        }),
        Action::Create => Err(ConfluxEngineError::analysis_failed(
            "Core Space contract creation unexpectedly reported sponsored gas",
        )),
    }
}

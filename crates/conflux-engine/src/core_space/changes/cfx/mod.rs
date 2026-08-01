mod collection;
mod verification;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use alloy_primitives::{Address, U256};
use contract_standards::Position;
use primitives::{Action, SignedTransaction};

pub(crate) use collection::collect_cfx_operations;
pub(crate) use verification::{read_cfx_state_values, verify_cfx_changes};

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

    /// Applies already-collected CFX operations that affect staking balances.
    pub(crate) fn apply_staking_balance_effects(
        &self,
        staking_balances: &mut BTreeMap<Address, U256>,
    ) -> Result<(), ConfluxEngineError> {
        for operation in &self.operations {
            match operation {
                CfxOperation::StakingDeposit {
                    account, amount, ..
                } => credit_staking_balance_if_present(staking_balances, *account, *amount)?,
                CfxOperation::StakingWithdrawal {
                    account,
                    principal_amount,
                    ..
                } => debit_staking_balance_if_present(
                    staking_balances,
                    *account,
                    *principal_amount,
                    "withdrawal",
                )?,
                CfxOperation::StakingBurn {
                    account, amount, ..
                } => debit_staking_balance_if_present(staking_balances, *account, *amount, "burn")?,
                _ => {}
            }
        }

        Ok(())
    }
}

fn credit_staking_balance_if_present(
    staking_balances: &mut BTreeMap<Address, U256>,
    account: Address,
    amount: U256,
) -> Result<(), ConfluxEngineError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_add(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "Core Space staking balance overflowed while replaying a deposit for {account}"
        ))
    })?;
    Ok(())
}

fn debit_staking_balance_if_present(
    staking_balances: &mut BTreeMap<Address, U256>,
    account: Address,
    amount: U256,
    operation: &str,
) -> Result<(), ConfluxEngineError> {
    let Some(balance) = staking_balances.get_mut(&account) else {
        return Ok(());
    };
    *balance = balance.checked_sub(amount).ok_or_else(|| {
        ConfluxEngineError::analysis_failed(format!(
            "Core Space staking balance underflowed while replaying a {operation} for {account}"
        ))
    })?;
    Ok(())
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

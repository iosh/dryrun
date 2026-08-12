use std::collections::BTreeMap;

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use cfx_types::AddressSpaceUtil;

use super::{NativeOperation, NativeOperations};
use crate::{
    espace::{EspaceChange, EspaceChangesError, EspaceNativeChangeError, EspaceNativeCurrency},
    primitive::{address_to_cfx, u256_from_cfx},
};

use super::super::ChangeOccurrence;

pub(crate) type NativeBalances = BTreeMap<Address, U256>;

pub(super) fn read_native_balances(
    state: &State,
    operation: &'static str,
    native_operations: &NativeOperations,
) -> Result<NativeBalances, EspaceChangesError> {
    native_operations
        .balance_accounts
        .iter()
        .map(|&address| {
            let balance = state
                .balance(&address_to_cfx(address).with_evm_space())
                .map_err(|error| EspaceChangesError::StateRead {
                    operation,
                    source: error,
                })?;
            Ok((address, u256_from_cfx(balance)))
        })
        .collect()
}

pub(super) fn verify_native_changes(
    native_operations: &NativeOperations,
    before_balances: &NativeBalances,
    after_balances: &NativeBalances,
    successful: bool,
    currency: &EspaceNativeCurrency,
) -> Result<Vec<ChangeOccurrence>, EspaceNativeChangeError> {
    let mut replayed_balances = before_balances.clone();
    let mut changes = Vec::new();

    for operation in &native_operations.operations {
        match operation {
            NativeOperation::AccountTransfer {
                position,
                from,
                to,
                amount,
            } => {
                decrease_balance(&mut replayed_balances, *from, *amount)?;
                increase_balance(&mut replayed_balances, *to, *amount)?;
                changes.push(ChangeOccurrence::new(
                    *position,
                    EspaceChange::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        currency: currency.clone(),
                    },
                ));
            }
            NativeOperation::SelfDestructBurn {
                position,
                contract,
                amount,
            } => {
                decrease_balance(&mut replayed_balances, *contract, *amount)?;
                changes.push(ChangeOccurrence::new(
                    *position,
                    EspaceChange::SelfDestructBurn {
                        contract_address: *contract,
                        raw_amount: *amount,
                        currency: currency.clone(),
                    },
                ));
            }
            NativeOperation::GasPrecharge { payer, amount } => {
                decrease_balance(&mut replayed_balances, *payer, *amount)?;
            }
            NativeOperation::GasRefund { recipient, amount } => {
                increase_balance(&mut replayed_balances, *recipient, *amount)?;
            }
        }
    }

    for &address in &native_operations.balance_accounts {
        let replayed = replayed_balances
            .get(&address)
            .copied()
            .ok_or(EspaceNativeChangeError::BalanceMissing { address })?;
        let actual = after_balances
            .get(&address)
            .copied()
            .ok_or(EspaceNativeChangeError::BalanceMissing { address })?;
        if replayed != actual {
            return Err(EspaceNativeChangeError::BalanceMismatch {
                address,
                replayed,
                actual,
            });
        }
    }

    if !successful && !changes.is_empty() {
        return Err(EspaceNativeChangeError::BusinessEffectOnFailedExecution);
    }

    Ok(changes)
}

fn decrease_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), EspaceNativeChangeError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(EspaceNativeChangeError::BalanceMissing { address })?;
    let current = *balance;
    *balance = current
        .checked_sub(amount)
        .ok_or(EspaceNativeChangeError::BalanceUnderflow {
            address,
            balance: current,
            amount,
        })?;
    Ok(())
}

fn increase_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), EspaceNativeChangeError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(EspaceNativeChangeError::BalanceMissing { address })?;
    let current = *balance;
    *balance = current
        .checked_add(amount)
        .ok_or(EspaceNativeChangeError::BalanceOverflow {
            address,
            balance: current,
            amount,
        })?;
    Ok(())
}

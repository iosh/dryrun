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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use alloy_primitives::{Address, U256};

    use super::verify_native_changes;
    use crate::espace::{EspaceNativeChangeError, EspaceNativeCurrency};

    use super::super::{NativeOperation, NativeOperations};

    #[test]
    fn rejects_native_balance_mismatch_and_business_effects_on_failure() {
        let sender = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);
        let operations = NativeOperations::from_operations(vec![
            NativeOperation::AccountTransfer {
                position: 1,
                from: sender,
                to: recipient,
                amount: U256::from(10),
            },
            NativeOperation::GasPrecharge {
                payer: sender,
                amount: U256::from(30),
            },
            NativeOperation::GasRefund {
                recipient: sender,
                amount: U256::from(7),
            },
        ]);
        let before = BTreeMap::from([(sender, U256::from(100)), (recipient, U256::ZERO)]);
        let after = BTreeMap::from([(sender, U256::from(67)), (recipient, U256::from(10))]);
        let currency = EspaceNativeCurrency {
            name: "Conflux".to_owned(),
            symbol: "CFX".to_owned(),
            decimals: 18,
        };

        verify_native_changes(&operations, &before, &after, true, &currency)
            .expect("complete replay should reconcile");

        let mut mismatched = after.clone();
        mismatched.insert(recipient, U256::from(11));
        assert!(matches!(
            verify_native_changes(&operations, &before, &mismatched, true, &currency),
            Err(EspaceNativeChangeError::BalanceMismatch { address, .. })
                if address == recipient
        ));
        assert!(matches!(
            verify_native_changes(&operations, &before, &after, false, &currency),
            Err(EspaceNativeChangeError::BusinessEffectOnFailedExecution)
        ));
    }
}

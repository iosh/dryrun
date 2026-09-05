use std::collections::BTreeMap;

use super::{NativeOperation, NativeOperations, NativeResolverDiagnostic};
use crate::{
    espace::EspaceStateReader,
    espace::{EspaceChange, EspaceChangesError, EspaceNativeCurrency},
};
use alloy_primitives::{Address, U256};

use super::super::ChangeOccurrence;

pub(crate) type NativeBalances = BTreeMap<Address, U256>;

pub(super) fn read_native_balances(
    state: &EspaceStateReader,
    operation: &'static str,
    native_operations: &NativeOperations,
) -> Result<NativeBalances, EspaceChangesError> {
    native_operations
        .balance_accounts
        .iter()
        .map(|&address| {
            let balance =
                state
                    .native_balance(address)
                    .map_err(|error| EspaceChangesError::StateAccess {
                        details: format!("{operation}: {error}"),
                    })?;
            Ok((address, balance))
        })
        .collect()
}

pub(super) fn verify_native_changes(
    native_operations: &NativeOperations,
    before_balances: &NativeBalances,
    after_balances: &NativeBalances,
    currency: &EspaceNativeCurrency,
) -> Result<Vec<ChangeOccurrence>, NativeResolverDiagnostic> {
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
            NativeOperation::GasPrecharge { payer, amount, .. } => {
                decrease_balance(&mut replayed_balances, *payer, *amount)?;
            }
            NativeOperation::GasRefund {
                recipient, amount, ..
            } => {
                increase_balance(&mut replayed_balances, *recipient, *amount)?;
            }
        }
    }

    for &address in &native_operations.balance_accounts {
        let replayed = replayed_balances[&address];
        let actual = after_balances[&address];
        if replayed != actual {
            return Err(NativeResolverDiagnostic::new(format!(
                "finalized native balance differs from replay for {address}: replayed {replayed}, state {actual}"
            )));
        }
    }

    Ok(changes)
}

fn decrease_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), NativeResolverDiagnostic> {
    let balance = balances
        .get_mut(&address)
        .unwrap_or_else(|| unreachable!("native replay account set is complete"));
    let current = *balance;
    *balance = current.checked_sub(amount).ok_or_else(|| {
        NativeResolverDiagnostic::new(format!(
            "native balance replay violated the execution invariant for {address}"
        ))
    })?;
    Ok(())
}

fn increase_balance(
    balances: &mut NativeBalances,
    address: Address,
    amount: U256,
) -> Result<(), NativeResolverDiagnostic> {
    let balance = balances
        .get_mut(&address)
        .unwrap_or_else(|| unreachable!("native replay account set is complete"));
    let current = *balance;
    *balance = current.checked_add(amount).ok_or_else(|| {
        NativeResolverDiagnostic::new(format!(
            "native balance replay violated the execution invariant for {address}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use alloy_primitives::{Address, U256};

    use super::verify_native_changes;
    use crate::espace::EspaceNativeCurrency;

    use super::super::{NativeOperation, NativeOperations};

    #[test]
    fn rejects_native_balance_mismatch() {
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
                position: 2,
                payer: sender,
                amount: U256::from(30),
            },
            NativeOperation::GasRefund {
                position: 3,
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

        verify_native_changes(&operations, &before, &after, &currency)
            .expect("complete replay should reconcile");

        let mut mismatched = after.clone();
        mismatched.insert(recipient, U256::from(11));
        assert!(verify_native_changes(&operations, &before, &mismatched, &currency).is_err());
    }
}

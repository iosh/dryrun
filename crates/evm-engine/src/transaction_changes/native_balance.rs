use std::collections::HashMap;

use alloy_primitives::{Address, U256};
use revm::state::EvmState;

use crate::{Change, NativeMetadata};

use super::{
    PositionedChange,
    candidate::{ChangeCandidate, ChangeCandidateKind},
    error::TransactionChangesError,
};

pub(crate) fn check_native_balances(
    state: &EvmState,
    candidates: &[ChangeCandidate],
    caller: Address,
    beneficiary: Address,
    gas_precharge: U256,
    caller_refund: U256,
    beneficiary_reward: U256,
) -> Result<Vec<PositionedChange>, TransactionChangesError> {
    let mut balances = state
        .iter()
        .map(|(address, account)| (*address, account.original_info.balance))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();

    decrease_balance(&mut balances, caller, gas_precharge)?;

    for candidate in candidates {
        let ChangeCandidateKind::NativeTransfer { from, to, amount } = candidate.kind else {
            continue;
        };

        decrease_balance(&mut balances, from, amount)?;
        increase_balance(&mut balances, to, amount)?;

        if !amount.is_zero() {
            changes.push(PositionedChange::new(
                candidate.position,
                Change::NativeTransfer {
                    from,
                    to,
                    raw_amount: amount,
                    metadata: NativeMetadata::default(),
                },
            ));
        }
    }

    increase_balance(&mut balances, caller, caller_refund)?;
    increase_balance(&mut balances, beneficiary, beneficiary_reward)?;

    for (address, account) in state {
        let replayed_balance = balances
            .get(address)
            .copied()
            .ok_or(TransactionChangesError::NativeAccountMissing { address: *address })?;

        let state_balance = if account.is_selfdestructed() {
            U256::ZERO
        } else {
            account.info.balance
        };

        if replayed_balance != state_balance {
            return Err(TransactionChangesError::NativeBalanceMismatch {
                address: *address,
                replayed_balance,
                state_balance,
            });
        }
    }

    Ok(changes)
}

fn decrease_balance(
    balances: &mut HashMap<Address, U256>,
    address: Address,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(TransactionChangesError::NativeAccountMissing { address })?;

    let current = *balance;
    *balance =
        current
            .checked_sub(amount)
            .ok_or(TransactionChangesError::NativeBalanceUnderflow {
                address,
                balance: current,
                amount,
            })?;

    Ok(())
}

fn increase_balance(
    balances: &mut HashMap<Address, U256>,
    address: Address,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(TransactionChangesError::NativeAccountMissing { address })?;

    let current = *balance;
    *balance =
        current
            .checked_add(amount)
            .ok_or(TransactionChangesError::NativeBalanceOverflow {
                address,
                balance: current,
                amount,
            })?;

    Ok(())
}

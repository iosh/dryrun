use std::collections::HashMap;

use alloy::primitives::{Address, U256};
use revm::state::EvmState;

use crate::{
    EvmChange, EvmExecutionEvent, EvmNativeChangeError, NativeCurrency, changes::ChangeOccurrence,
    execution::EvmFeeSettlement,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeCandidate {
    Transfer {
        event_index: usize,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        event_index: usize,
        contract: Address,
        amount: U256,
    },
}

pub(super) fn analyze_native_changes(
    state: &EvmState,
    events: &[EvmExecutionEvent],
    caller: Address,
    beneficiary: Address,
    fee_settlement: &EvmFeeSettlement,
    currency: &NativeCurrency,
) -> Result<Vec<ChangeOccurrence>, EvmNativeChangeError> {
    let candidates = collect_native_candidates(state, events)?;

    check_native_balances(
        state,
        &candidates,
        caller,
        beneficiary,
        fee_settlement,
        currency,
    )
}

fn collect_native_candidates(
    state: &EvmState,
    events: &[EvmExecutionEvent],
) -> Result<Vec<NativeCandidate>, EvmNativeChangeError> {
    let mut candidates = Vec::new();

    for (event_index, event) in events.iter().enumerate() {
        let candidate = match event {
            EvmExecutionEvent::Call {
                caller,
                target,
                value,
            } if !value.is_zero() => Some(NativeCandidate::Transfer {
                event_index,
                from: *caller,
                to: *target,
                amount: *value,
            }),
            EvmExecutionEvent::CreateTransfer { from, to, amount } if !amount.is_zero() => {
                Some(NativeCandidate::Transfer {
                    event_index,
                    from: *from,
                    to: *to,
                    amount: *amount,
                })
            }
            EvmExecutionEvent::SelfDestruct { amount, .. } if amount.is_zero() => None,
            EvmExecutionEvent::SelfDestruct {
                contract,
                target,
                amount,
            } if contract == target => {
                let account = state
                    .get(contract)
                    .ok_or(EvmNativeChangeError::AccountMissing { address: *contract })?;

                account
                    .is_selfdestructed()
                    .then_some(NativeCandidate::SelfDestructBurn {
                        event_index,
                        contract: *contract,
                        amount: *amount,
                    })
            }
            EvmExecutionEvent::SelfDestruct {
                contract,
                target,
                amount,
            } => Some(NativeCandidate::Transfer {
                event_index,
                from: *contract,
                to: *target,
                amount: *amount,
            }),
            EvmExecutionEvent::Call { .. }
            | EvmExecutionEvent::CreateTransfer { .. }
            | EvmExecutionEvent::Log { .. } => None,
        };

        if let Some(candidate) = candidate {
            candidates.push(candidate);
        }
    }

    Ok(candidates)
}

fn check_native_balances(
    state: &EvmState,
    candidates: &[NativeCandidate],
    caller: Address,
    beneficiary: Address,
    fee_settlement: &EvmFeeSettlement,
    currency: &NativeCurrency,
) -> Result<Vec<ChangeOccurrence>, EvmNativeChangeError> {
    let mut balances = state
        .iter()
        .map(|(address, account)| (*address, account.original_info.balance))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();

    decrease_balance(&mut balances, caller, fee_settlement.caller_precharge())?;

    for candidate in candidates {
        match candidate {
            NativeCandidate::Transfer {
                event_index,
                from,
                to,
                amount,
            } => {
                decrease_balance(&mut balances, *from, *amount)?;
                increase_balance(&mut balances, *to, *amount)?;
                changes.push(ChangeOccurrence::new(
                    *event_index,
                    EvmChange::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        currency: currency.clone(),
                    },
                ));
            }
            NativeCandidate::SelfDestructBurn {
                event_index,
                contract,
                amount,
            } => {
                decrease_balance(&mut balances, *contract, *amount)?;
                changes.push(ChangeOccurrence::new(
                    *event_index,
                    EvmChange::SelfDestructBurn {
                        contract_address: *contract,
                        raw_amount: *amount,
                        currency: currency.clone(),
                    },
                ));
            }
        }
    }

    increase_balance(&mut balances, caller, fee_settlement.caller_refund())?;
    increase_balance(
        &mut balances,
        beneficiary,
        fee_settlement.beneficiary_reward(),
    )?;

    for (address, account) in state {
        let replayed_balance = balances
            .get(address)
            .copied()
            .ok_or(EvmNativeChangeError::AccountMissing { address: *address })?;

        let state_balance = if account.is_selfdestructed() {
            U256::ZERO
        } else {
            account.info.balance
        };

        if replayed_balance != state_balance {
            return Err(EvmNativeChangeError::BalanceMismatch {
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
) -> Result<(), EvmNativeChangeError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(EvmNativeChangeError::AccountMissing { address })?;

    let current = *balance;
    *balance = current
        .checked_sub(amount)
        .ok_or(EvmNativeChangeError::BalanceUnderflow {
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
) -> Result<(), EvmNativeChangeError> {
    let balance = balances
        .get_mut(&address)
        .ok_or(EvmNativeChangeError::AccountMissing { address })?;

    let current = *balance;
    *balance = current
        .checked_add(amount)
        .ok_or(EvmNativeChangeError::BalanceOverflow {
            address,
            balance: current,
            amount,
        })?;

    Ok(())
}

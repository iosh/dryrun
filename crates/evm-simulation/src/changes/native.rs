use std::collections::HashMap;

use alloy::primitives::{Address, U256};
use contract_standards::legacy::Position;
use revm::state::EvmState;
use simulation_changes::{Change, NativeMetadata, PositionedChange};

use crate::{EvmExecutionObservation, EvmNativeChangeError, execution::EvmFeeSettlement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeCandidate {
    Transfer {
        position: Position,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: Position,
        contract: Address,
        amount: U256,
    },
}

pub(crate) fn analyze_native_changes(
    state: &EvmState,
    observations: &[EvmExecutionObservation],
    caller: Address,
    beneficiary: Address,
    fee_settlement: &EvmFeeSettlement,
) -> Result<Vec<PositionedChange>, EvmNativeChangeError> {
    let candidates = collect_native_candidates(state, observations)?;

    check_native_balances(
        state,
        &candidates,
        caller,
        beneficiary,
        fee_settlement.caller_precharge(),
        fee_settlement.caller_refund(),
        fee_settlement.beneficiary_reward(),
    )
}

fn collect_native_candidates(
    state: &EvmState,
    observations: &[EvmExecutionObservation],
) -> Result<Vec<NativeCandidate>, EvmNativeChangeError> {
    let mut candidates = Vec::new();

    for (observation_index, observation) in observations.iter().enumerate() {
        let candidate = match observation {
            EvmExecutionObservation::Call {
                caller,
                target,
                value,
                ..
            } if !value.is_zero() => Some(NativeCandidate::Transfer {
                position: Position::new(observation_index, 0),
                from: *caller,
                to: *target,
                amount: *value,
            }),
            EvmExecutionObservation::CreateTransfer { from, to, amount } if !amount.is_zero() => {
                Some(NativeCandidate::Transfer {
                    position: Position::new(observation_index, 0),
                    from: *from,
                    to: *to,
                    amount: *amount,
                })
            }
            EvmExecutionObservation::SelfDestruct { amount, .. } if amount.is_zero() => None,
            EvmExecutionObservation::SelfDestruct {
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
                        position: Position::new(observation_index, 0),
                        contract: *contract,
                        amount: *amount,
                    })
            }
            EvmExecutionObservation::SelfDestruct {
                contract,
                target,
                amount,
            } => Some(NativeCandidate::Transfer {
                position: Position::new(observation_index, 0),
                from: *contract,
                to: *target,
                amount: *amount,
            }),
            EvmExecutionObservation::Call { .. }
            | EvmExecutionObservation::CreateTransfer { .. }
            | EvmExecutionObservation::Log { .. } => None,
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
    caller_precharge: U256,
    caller_refund: U256,
    beneficiary_reward: U256,
) -> Result<Vec<PositionedChange>, EvmNativeChangeError> {
    let mut balances = state
        .iter()
        .map(|(address, account)| (*address, account.original_info.balance))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();

    decrease_balance(&mut balances, caller, caller_precharge)?;

    for candidate in candidates {
        match candidate {
            NativeCandidate::Transfer {
                position,
                from,
                to,
                amount,
            } => {
                decrease_balance(&mut balances, *from, *amount)?;
                increase_balance(&mut balances, *to, *amount)?;
                changes.push(PositionedChange::new(
                    *position,
                    Change::NativeTransfer {
                        from: *from,
                        to: *to,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
                    },
                ));
            }
            NativeCandidate::SelfDestructBurn {
                position,
                contract,
                amount,
            } => {
                decrease_balance(&mut balances, *contract, *amount)?;
                changes.push(PositionedChange::new(
                    *position,
                    Change::SelfDestructBurn {
                        contract_address: *contract,
                        raw_amount: *amount,
                        metadata: NativeMetadata::default(),
                    },
                ));
            }
        }
    }

    increase_balance(&mut balances, caller, caller_refund)?;
    increase_balance(&mut balances, beneficiary, beneficiary_reward)?;

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

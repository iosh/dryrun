use std::collections::HashMap;

use alloy::primitives::{Address, U256};
use contract_standards::Position;
use revm::state::EvmState;
use simulation_changes::{Change, NativeMetadata, PositionedChange};

use crate::{
    EvmExecutionObservation, EvmExecutionObserver, EvmExecutionOutput, EvmNativeChangeError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeCandidate {
    position: Position,
    from: Address,
    to: Address,
    amount: U256,
}

pub fn analyze_native_changes(
    output: &EvmExecutionOutput<EvmExecutionObserver>,
) -> Result<Vec<PositionedChange>, EvmNativeChangeError> {
    let observations = output.observations();
    let candidates = collect_native_candidates(&observations)?;
    let fee_settlement = output.fee_settlement();

    check_native_balances(
        output
            .transition()
            .map_err(|_| EvmNativeChangeError::TransitionUnavailable)?,
        &candidates,
        output.caller(),
        output.beneficiary(),
        fee_settlement.gas_precharge,
        fee_settlement.caller_refund,
        fee_settlement.beneficiary_reward,
    )
}

fn collect_native_candidates(
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
            } if !value.is_zero() => Some(NativeCandidate {
                position: Position::new(observation_index, 0),
                from: *caller,
                to: *target,
                amount: *value,
            }),
            EvmExecutionObservation::CreateTransfer { from, to, amount } if !amount.is_zero() => {
                Some(NativeCandidate {
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
                return Err(EvmNativeChangeError::UnsupportedSelfDestructToSelf {
                    observation_index,
                    contract: *contract,
                    amount: *amount,
                });
            }
            EvmExecutionObservation::SelfDestruct {
                contract,
                target,
                amount,
            } => Some(NativeCandidate {
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
    gas_precharge: U256,
    caller_refund: U256,
    beneficiary_reward: U256,
) -> Result<Vec<PositionedChange>, EvmNativeChangeError> {
    let mut balances = state
        .iter()
        .map(|(address, account)| (*address, account.original_info.balance))
        .collect::<HashMap<_, _>>();
    let mut changes = Vec::new();

    decrease_balance(&mut balances, caller, gas_precharge)?;

    for candidate in candidates {
        decrease_balance(&mut balances, candidate.from, candidate.amount)?;
        increase_balance(&mut balances, candidate.to, candidate.amount)?;

        if !candidate.amount.is_zero() {
            changes.push(PositionedChange::new(
                candidate.position,
                Change::NativeTransfer {
                    from: candidate.from,
                    to: candidate.to,
                    raw_amount: candidate.amount,
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
            .ok_or(EvmNativeChangeError::NativeAccountMissing { address: *address })?;

        let state_balance = if account.is_selfdestructed() {
            U256::ZERO
        } else {
            account.info.balance
        };

        if replayed_balance != state_balance {
            return Err(EvmNativeChangeError::NativeBalanceMismatch {
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
        .ok_or(EvmNativeChangeError::NativeAccountMissing { address })?;

    let current = *balance;
    *balance = current
        .checked_sub(amount)
        .ok_or(EvmNativeChangeError::NativeBalanceUnderflow {
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
        .ok_or(EvmNativeChangeError::NativeAccountMissing { address })?;

    let current = *balance;
    *balance = current
        .checked_add(amount)
        .ok_or(EvmNativeChangeError::NativeBalanceOverflow {
            address,
            balance: current,
            amount,
        })?;

    Ok(())
}

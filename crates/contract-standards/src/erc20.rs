//! ERC-20 transaction state checks.

use std::collections::{HashMap, hash_map::Entry};

use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc20AllowanceKey, Erc20BalanceKey, Position, StandardCandidate,
    StandardCandidateKind, StandardStateValues, StateArithmeticOperation, StatePhase,
    StateRequirement, StateRequirements,
    candidate::AllowanceSource,
    change::legacy::{Change, PositionedChange},
};

struct Erc20Replay {
    balances: HashMap<Erc20BalanceKey, U256>,
    total_supplies: HashMap<Address, U256>,
}

#[derive(Debug, Clone, Copy)]
struct PositionedAllowance {
    position: Position,
    source: AllowanceSource,
}

pub(crate) fn check_erc20_changes(
    candidates: &[StandardCandidate],
    keys: &StateRequirements,
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedChange>, ContractStandardsError> {
    let mut changes = check_erc20_movements(candidates, keys, before, after)?;
    changes.extend(check_erc20_allowances(candidates, before, after)?);

    Ok(changes)
}

pub(crate) fn check_erc20_movements(
    candidates: &[StandardCandidate],
    keys: &StateRequirements,
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedChange>, ContractStandardsError> {
    let replayed = replay_erc20_movements(candidates, before)?;

    for &key in &keys.erc20_balances {
        let replayed_balance = replayed.balances.get(&key).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20Balance(key),
                phase: StatePhase::Before,
            },
        )?;

        let after_balance = after.erc20_balances.get(&key).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20Balance(key),
                phase: StatePhase::After,
            },
        )?;

        if replayed_balance != after_balance {
            return Err(ContractStandardsError::Erc20BalanceMismatch {
                token: key.token,
                account: key.account,
                replayed_balance,
                after_balance,
            });
        }
    }

    for &token in &keys.erc20_total_supplies {
        let replayed_total_supply = replayed.total_supplies.get(&token).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20TotalSupply(token),
                phase: StatePhase::Before,
            },
        )?;

        let after_total_supply = after.erc20_total_supplies.get(&token).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20TotalSupply(token),
                phase: StatePhase::After,
            },
        )?;

        if replayed_total_supply != after_total_supply {
            return Err(ContractStandardsError::Erc20TotalSupplyMismatch {
                token,
                replayed_total_supply,
                after_total_supply,
            });
        }
    }

    Ok(candidates
        .iter()
        .filter_map(erc20_movement_change)
        .collect())
}

pub(crate) fn check_erc20_allowances(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedChange>, ContractStandardsError> {
    let allowances = collect_allowances(candidates);
    let mut changes = Vec::new();

    for (key, allowance) in allowances {
        let before_allowance = allowance_value(before, key, StatePhase::Before)?;
        let after_allowance = allowance_value(after, key, StatePhase::After)?;

        match allowance.source {
            AllowanceSource::ApprovalEvent { value } if value != after_allowance => {
                return Err(ContractStandardsError::Erc20ApprovalValueMismatch {
                    token: key.token,
                    owner: key.owner,
                    spender: key.spender,
                    event_value: value,
                    after_allowance,
                });
            }
            AllowanceSource::ApprovalEvent { .. } | AllowanceSource::TransferFromCall { .. } => {}
        }

        if before_allowance != after_allowance {
            changes.push(PositionedChange::new(
                allowance.position,
                Change::Erc20Allowance {
                    contract_address: key.token,
                    owner: key.owner,
                    spender: key.spender,
                    raw_amount_before: before_allowance,
                    raw_amount_after: after_allowance,
                },
            ));
        }
    }

    Ok(changes)
}

fn allowance_value(
    values: &StandardStateValues,
    key: Erc20AllowanceKey,
    phase: StatePhase,
) -> Result<U256, ContractStandardsError> {
    values
        .erc20_allowances
        .get(&key)
        .copied()
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc20Allowance(key),
            phase,
        })
}

fn collect_allowances(
    candidates: &[StandardCandidate],
) -> Vec<(Erc20AllowanceKey, PositionedAllowance)> {
    let mut allowance_indexes: HashMap<Erc20AllowanceKey, usize> = HashMap::new();
    let mut allowances: Vec<(Erc20AllowanceKey, PositionedAllowance)> = Vec::new();

    for candidate in candidates {
        let StandardCandidateKind::Erc20Allowance {
            token,
            owner,
            spender,
            source,
        } = candidate.kind
        else {
            continue;
        };

        let key = Erc20AllowanceKey {
            token,
            owner,
            spender,
        };
        let positioned = PositionedAllowance {
            position: candidate.position,
            source,
        };

        match allowance_indexes.entry(key) {
            Entry::Occupied(entry) => allowances[*entry.get()].1 = positioned,
            Entry::Vacant(entry) => {
                entry.insert(allowances.len());
                allowances.push((key, positioned));
            }
        }
    }

    allowances
}

fn erc20_movement_change(candidate: &StandardCandidate) -> Option<PositionedChange> {
    let StandardCandidateKind::Erc20Movement {
        token,
        from,
        to,
        amount,
    } = candidate.kind
    else {
        return None;
    };

    if amount.is_zero() {
        return None;
    }

    let change = if from == Address::ZERO {
        Change::Erc20Mint {
            contract_address: token,
            to,
            raw_amount: amount,
        }
    } else if to == Address::ZERO {
        Change::Erc20Burn {
            contract_address: token,
            from,
            raw_amount: amount,
        }
    } else {
        Change::Erc20Transfer {
            contract_address: token,
            from,
            to,
            raw_amount: amount,
        }
    };

    Some(PositionedChange::new(candidate.position, change))
}

fn replay_erc20_movements(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
) -> Result<Erc20Replay, ContractStandardsError> {
    let mut balances = before.erc20_balances.clone();
    let mut total_supplies = before.erc20_total_supplies.clone();

    for candidate in candidates {
        let StandardCandidateKind::Erc20Movement {
            token,
            from,
            to,
            amount,
        } = candidate.kind
        else {
            continue;
        };

        match (from == Address::ZERO, to == Address::ZERO) {
            (true, true) => {
                return Err(ContractStandardsError::Erc20TransferBetweenZeroAddresses {
                    token,
                    amount,
                });
            }

            (true, false) => {
                increase_balance(&mut balances, token, to, amount)?;
                increase_total_supply(&mut total_supplies, token, amount)?;
            }

            (false, true) => {
                decrease_balance(&mut balances, token, from, amount)?;
                decrease_total_supply(&mut total_supplies, token, amount)?;
            }

            (false, false) => {
                decrease_balance(&mut balances, token, from, amount)?;
                increase_balance(&mut balances, token, to, amount)?;
            }
        }
    }

    Ok(Erc20Replay {
        balances,
        total_supplies,
    })
}

fn decrease_balance(
    balances: &mut HashMap<Erc20BalanceKey, U256>,
    token: Address,
    account: Address,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let key = Erc20BalanceKey { token, account };
    let balance = balances
        .get_mut(&key)
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc20Balance(key),
            phase: StatePhase::Before,
        })?;

    let current = *balance;
    *balance = current.checked_sub(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc20Balance(key),
            StateArithmeticOperation::Subtract,
            current,
            amount,
        )
    })?;

    Ok(())
}

fn increase_balance(
    balances: &mut HashMap<Erc20BalanceKey, U256>,
    token: Address,
    account: Address,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let key = Erc20BalanceKey { token, account };
    let balance = balances
        .get_mut(&key)
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc20Balance(key),
            phase: StatePhase::Before,
        })?;

    let current = *balance;
    *balance = current.checked_add(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc20Balance(key),
            StateArithmeticOperation::Add,
            current,
            amount,
        )
    })?;

    Ok(())
}

fn decrease_total_supply(
    total_supplies: &mut HashMap<Address, U256>,
    token: Address,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let total_supply =
        total_supplies
            .get_mut(&token)
            .ok_or(ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20TotalSupply(token),
                phase: StatePhase::Before,
            })?;

    let current = *total_supply;
    *total_supply = current.checked_sub(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc20TotalSupply(token),
            StateArithmeticOperation::Subtract,
            current,
            amount,
        )
    })?;

    Ok(())
}

fn increase_total_supply(
    total_supplies: &mut HashMap<Address, U256>,
    token: Address,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let total_supply =
        total_supplies
            .get_mut(&token)
            .ok_or(ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::Erc20TotalSupply(token),
                phase: StatePhase::Before,
            })?;

    let current = *total_supply;
    *total_supply = current.checked_add(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc20TotalSupply(token),
            StateArithmeticOperation::Add,
            current,
            amount,
        )
    })?;

    Ok(())
}

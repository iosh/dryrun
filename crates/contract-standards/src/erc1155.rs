//! ERC-1155 transaction state checks.

use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc1155BalanceKey, StandardCandidate, StandardCandidateKind,
    StandardStateValues, StateArithmeticOperation, StatePhase, StateRequirement, StateRequirements,
    change::legacy::{Change, PositionedChange},
};

pub(crate) fn check_erc1155_movements(
    candidates: &[StandardCandidate],
    keys: &StateRequirements,
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedChange>, ContractStandardsError> {
    let replayed_balances = replay_erc1155_movements(candidates, before)?;

    for &key in &keys.erc1155_balances {
        let replayed_balance = balance_value(&replayed_balances, key, StatePhase::Before)?;
        let after_balance = balance_value(&after.erc1155_balances, key, StatePhase::After)?;

        if replayed_balance != after_balance {
            return Err(ContractStandardsError::Erc1155BalanceMismatch {
                collection: key.collection,
                account: key.account,
                token_id: key.token_id,
                replayed_balance,
            });
        }
    }

    Ok(candidates
        .iter()
        .filter_map(erc1155_movement_change)
        .collect())
}

fn erc1155_movement_change(candidate: &StandardCandidate) -> Option<PositionedChange> {
    let StandardCandidateKind::Erc1155Transfer {
        collection,
        from,
        to,
        token_id,
        amount,
    } = candidate.kind
    else {
        return None;
    };

    if amount.is_zero() {
        return None;
    }

    let change = if from == Address::ZERO {
        Change::Erc1155Mint {
            contract_address: collection,
            to,
            token_id,
            raw_amount: amount,
        }
    } else if to == Address::ZERO {
        Change::Erc1155Burn {
            contract_address: collection,
            from,
            token_id,
            raw_amount: amount,
        }
    } else {
        Change::Erc1155Transfer {
            contract_address: collection,
            from,
            to,
            token_id,
            raw_amount: amount,
        }
    };

    Some(PositionedChange::new(candidate.position, change))
}

fn replay_erc1155_movements(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
) -> Result<HashMap<Erc1155BalanceKey, U256>, ContractStandardsError> {
    let mut balances = before.erc1155_balances.clone();

    for candidate in candidates {
        let StandardCandidateKind::Erc1155Transfer {
            collection,
            from,
            to,
            token_id,
            amount,
        } = candidate.kind
        else {
            continue;
        };

        match (from == Address::ZERO, to == Address::ZERO) {
            (true, true) if amount == U256::ZERO => {}

            (true, true) => {
                return Err(
                    ContractStandardsError::Erc1155TransferBetweenZeroAddresses {
                        collection,
                        token_id,
                        amount,
                    },
                );
            }

            (true, false) => {
                add_to_balance(&mut balances, collection, to, token_id, amount)?;
            }

            (false, true) => {
                subtract_from_balance(&mut balances, collection, from, token_id, amount)?;
            }

            (false, false) => {
                subtract_from_balance(&mut balances, collection, from, token_id, amount)?;
                add_to_balance(&mut balances, collection, to, token_id, amount)?;
            }
        }
    }

    Ok(balances)
}

fn subtract_from_balance(
    balances: &mut HashMap<Erc1155BalanceKey, U256>,
    collection: Address,
    account: Address,
    token_id: U256,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let key = Erc1155BalanceKey {
        collection,
        account,
        token_id,
    };
    let balance = balances
        .get_mut(&key)
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc1155Balance(key),
            phase: StatePhase::Before,
        })?;

    let current = *balance;
    *balance = current.checked_sub(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc1155Balance(key),
            StateArithmeticOperation::Subtract,
            current,
            amount,
        )
    })?;

    Ok(())
}

fn add_to_balance(
    balances: &mut HashMap<Erc1155BalanceKey, U256>,
    collection: Address,
    account: Address,
    token_id: U256,
    amount: U256,
) -> Result<(), ContractStandardsError> {
    let key = Erc1155BalanceKey {
        collection,
        account,
        token_id,
    };
    let balance = balances
        .get_mut(&key)
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc1155Balance(key),
            phase: StatePhase::Before,
        })?;

    let current = *balance;
    *balance = current.checked_add(amount).ok_or_else(|| {
        ContractStandardsError::state_arithmetic(
            StateRequirement::Erc1155Balance(key),
            StateArithmeticOperation::Add,
            current,
            amount,
        )
    })?;

    Ok(())
}

fn balance_value(
    balances: &HashMap<Erc1155BalanceKey, U256>,
    key: Erc1155BalanceKey,
    phase: StatePhase,
) -> Result<U256, ContractStandardsError> {
    balances
        .get(&key)
        .copied()
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::Erc1155Balance(key),
            phase,
        })
}

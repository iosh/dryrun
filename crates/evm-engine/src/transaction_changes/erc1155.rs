//! ERC-1155 transaction state checks.

use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::Change;

use super::{
    PositionedChange,
    candidate::{ChangeCandidate, ChangeCandidateKind},
    error::TransactionChangesError,
    token_state::{Erc1155BalanceKey, TokenStateKeys, TokenStateValues},
};

pub(crate) fn check_erc1155_movements(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, TransactionChangesError> {
    let replayed_balances = replay_erc1155_movements(candidates, before)?;

    for &key in &keys.erc1155_balances {
        let replayed_balance = balance_value(&replayed_balances, key, "before")?;
        let after_balance = balance_value(&after.erc1155_balances, key, "after")?;

        if replayed_balance != after_balance {
            return Err(TransactionChangesError::Erc1155BalanceMismatch {
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

fn erc1155_movement_change(candidate: &ChangeCandidate) -> Option<PositionedChange> {
    let ChangeCandidateKind::Erc1155Transfer {
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
    candidates: &[ChangeCandidate],
    before: &TokenStateValues,
) -> Result<HashMap<Erc1155BalanceKey, U256>, TransactionChangesError> {
    let mut balances = before.erc1155_balances.clone();

    for candidate in candidates {
        let ChangeCandidateKind::Erc1155Transfer {
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
                    TransactionChangesError::Erc1155TransferBetweenZeroAddresses {
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
) -> Result<(), TransactionChangesError> {
    let key = Erc1155BalanceKey {
        collection,
        account,
        token_id,
    };
    let balance = balances
        .get_mut(&key)
        .ok_or(TransactionChangesError::Erc1155BalanceMissing {
            collection,
            account,
            token_id,
            state: "before",
        })?;

    let current = *balance;
    *balance =
        current
            .checked_sub(amount)
            .ok_or(TransactionChangesError::Erc1155BalanceUnderflow {
                collection,
                account,
                token_id,
                amount,
            })?;

    Ok(())
}

fn add_to_balance(
    balances: &mut HashMap<Erc1155BalanceKey, U256>,
    collection: Address,
    account: Address,
    token_id: U256,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let key = Erc1155BalanceKey {
        collection,
        account,
        token_id,
    };
    let balance = balances
        .get_mut(&key)
        .ok_or(TransactionChangesError::Erc1155BalanceMissing {
            collection,
            account,
            token_id,
            state: "before",
        })?;

    let current = *balance;
    *balance =
        current
            .checked_add(amount)
            .ok_or(TransactionChangesError::Erc1155BalanceOverflow {
                collection,
                account,
                token_id,
                amount,
            })?;

    Ok(())
}

fn balance_value(
    balances: &HashMap<Erc1155BalanceKey, U256>,
    key: Erc1155BalanceKey,
    state: &'static str,
) -> Result<U256, TransactionChangesError> {
    balances
        .get(&key)
        .copied()
        .ok_or(TransactionChangesError::Erc1155BalanceMissing {
            collection: key.collection,
            account: key.account,
            token_id: key.token_id,
            state,
        })
}

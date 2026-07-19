//! ERC-20 transaction state checks.

use std::collections::HashMap;

use super::{
    candidate::{ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence},
    error::TransactionChangesError,
    token_state::{Erc20AllowanceKey, Erc20BalanceKey, TokenStateKeys, TokenStateValues},
};
use alloy_primitives::{Address, U256};

struct Erc20Replay {
    balances: HashMap<Erc20BalanceKey, U256>,
    total_supplies: HashMap<Address, U256>,
}

pub(crate) fn check_erc20_changes(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    check_erc20_movements(candidates, keys, before, after)?;
    check_erc20_allowances(candidates, keys, before, after)?;

    Ok(())
}

pub(crate) fn check_erc20_movements(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    let replayed = replay_erc20_movements(candidates, before)?;

    for &key in &keys.erc20_balances {
        let replayed_balance = replayed.balances.get(&key).copied().ok_or(
            TransactionChangesError::Erc20BalanceMissing {
                token: key.token,
                account: key.account,
                state: "before",
            },
        )?;

        let after_balance = after.erc20_balances.get(&key).copied().ok_or(
            TransactionChangesError::Erc20BalanceMissing {
                token: key.token,
                account: key.account,
                state: "after",
            },
        )?;

        if replayed_balance != after_balance {
            return Err(TransactionChangesError::Erc20BalanceMismatch {
                token: key.token,
                account: key.account,
                replayed_balance,
                after_balance,
            });
        }
    }

    for &token in &keys.erc20_total_supplies {
        let replayed_total_supply = replayed.total_supplies.get(&token).copied().ok_or(
            TransactionChangesError::Erc20TotalSupplyMissing {
                token,
                state: "before",
            },
        )?;

        let after_total_supply = after.erc20_total_supplies.get(&token).copied().ok_or(
            TransactionChangesError::Erc20TotalSupplyMissing {
                token,
                state: "after",
            },
        )?;

        if replayed_total_supply != after_total_supply {
            return Err(TransactionChangesError::Erc20TotalSupplyMismatch {
                token,
                replayed_total_supply,
                after_total_supply,
            });
        }
    }

    Ok(())
}

pub(crate) fn check_erc20_allowances(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    let evidence_by_allowance = collect_last_allowance_evidence(candidates);

    for &key in &keys.erc20_allowances {
        allowance_value(before, key, "before")?;
        let after_allowance = allowance_value(after, key, "after")?;

        let evidence = evidence_by_allowance.get(&key).copied().ok_or(
            TransactionChangesError::Erc20AllowanceEvidenceMissing {
                token: key.token,
                owner: key.owner,
                spender: key.spender,
            },
        )?;

        match evidence {
            Erc20AllowanceEvidence::ApprovalEvent { value } if value != after_allowance => {
                return Err(TransactionChangesError::Erc20ApprovalValueMismatch {
                    token: key.token,
                    owner: key.owner,
                    spender: key.spender,
                    event_value: value,
                    after_allowance,
                });
            }
            Erc20AllowanceEvidence::ApprovalEvent { .. }
            | Erc20AllowanceEvidence::TransferFromCall { .. } => {}
        }
    }

    Ok(())
}

fn allowance_value(
    values: &TokenStateValues,
    key: Erc20AllowanceKey,
    state: &'static str,
) -> Result<U256, TransactionChangesError> {
    values.erc20_allowances.get(&key).copied().ok_or(
        TransactionChangesError::Erc20AllowanceMissing {
            token: key.token,
            owner: key.owner,
            spender: key.spender,
            state,
        },
    )
}

fn collect_last_allowance_evidence(
    candidates: &[ChangeCandidate],
) -> HashMap<Erc20AllowanceKey, Erc20AllowanceEvidence> {
    let mut evidence_by_allowance = HashMap::new();

    for candidate in candidates {
        let ChangeCandidateKind::Erc20Allowance {
            token,
            owner,
            spender,
            evidence,
        } = candidate.kind
        else {
            continue;
        };

        evidence_by_allowance.insert(
            Erc20AllowanceKey {
                token,
                owner,
                spender,
            },
            evidence,
        );
    }

    evidence_by_allowance
}

fn replay_erc20_movements(
    candidates: &[ChangeCandidate],
    before: &TokenStateValues,
) -> Result<Erc20Replay, TransactionChangesError> {
    let mut balances = before.erc20_balances.clone();
    let mut total_supplies = before.erc20_total_supplies.clone();

    for candidate in candidates {
        let ChangeCandidateKind::Erc20Transfer {
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
                return Err(TransactionChangesError::Erc20TransferBetweenZeroAddresses {
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
) -> Result<(), TransactionChangesError> {
    let key = Erc20BalanceKey { token, account };
    let balance = balances
        .get_mut(&key)
        .ok_or(TransactionChangesError::Erc20BalanceMissing {
            token,
            account,
            state: "before",
        })?;

    let current = *balance;
    *balance =
        current
            .checked_sub(amount)
            .ok_or(TransactionChangesError::Erc20BalanceUnderflow {
                token,
                account,
                balance: current,
                amount,
            })?;

    Ok(())
}

fn increase_balance(
    balances: &mut HashMap<Erc20BalanceKey, U256>,
    token: Address,
    account: Address,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let key = Erc20BalanceKey { token, account };
    let balance = balances
        .get_mut(&key)
        .ok_or(TransactionChangesError::Erc20BalanceMissing {
            token,
            account,
            state: "before",
        })?;

    let current = *balance;
    *balance =
        current
            .checked_add(amount)
            .ok_or(TransactionChangesError::Erc20BalanceOverflow {
                token,
                account,
                balance: current,
                amount,
            })?;

    Ok(())
}

fn decrease_total_supply(
    total_supplies: &mut HashMap<Address, U256>,
    token: Address,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let total_supply =
        total_supplies
            .get_mut(&token)
            .ok_or(TransactionChangesError::Erc20TotalSupplyMissing {
                token,
                state: "before",
            })?;

    let current = *total_supply;
    *total_supply =
        current
            .checked_sub(amount)
            .ok_or(TransactionChangesError::Erc20TotalSupplyUnderflow {
                token,
                total_supply: current,
                amount,
            })?;

    Ok(())
}

fn increase_total_supply(
    total_supplies: &mut HashMap<Address, U256>,
    token: Address,
    amount: U256,
) -> Result<(), TransactionChangesError> {
    let total_supply =
        total_supplies
            .get_mut(&token)
            .ok_or(TransactionChangesError::Erc20TotalSupplyMissing {
                token,
                state: "before",
            })?;

    let current = *total_supply;
    *total_supply =
        current
            .checked_add(amount)
            .ok_or(TransactionChangesError::Erc20TotalSupplyOverflow {
                token,
                total_supply: current,
                amount,
            })?;

    Ok(())
}

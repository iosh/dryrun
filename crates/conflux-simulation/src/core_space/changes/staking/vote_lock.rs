use std::collections::{BTreeMap, btree_map::Entry};

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use primitives::VoteStakeList;

use super::CommittedStakingCall;
use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::cfx::CfxStateValues,
    core_space::changes::{PendingCoreSpaceChange, PositionedCoreSpaceChange},
    primitive::{address_to_cfx, u256_from_cfx, u256_to_cfx},
    state::RecordedVoteLists,
};

pub(crate) fn verify_vote_lock_changes(
    post_state: &State,
    committed_staking_calls: &[CommittedStakingCall],
    vote_lists: &RecordedVoteLists,
    current_block_number: u64,
    before_cfx_state: &CfxStateValues,
) -> Result<Vec<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    let mut vote_lists_by_account = BTreeMap::new();
    let mut staking_balances_by_account = BTreeMap::new();
    let mut positioned_changes = Vec::new();

    for committed_call in committed_staking_calls {
        let account = committed_call.account();
        let staking_balance = match staking_balances_by_account.entry(account) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(before_cfx_state.staking_balance(account)?),
        };
        match *committed_call {
            CommittedStakingCall::Deposit { amount, .. } => {
                *staking_balance = staking_balance.checked_add(amount).ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space staking balance overflowed at a deposit for {account}"
                    ))
                })?;
                continue;
            }
            CommittedStakingCall::Withdrawal { .. } | CommittedStakingCall::VoteLock { .. } => {}
        }

        let vote_list = match vote_lists_by_account.entry(account) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let vote_list =
                    vote_lists
                        .for_account(address_to_cfx(account))
                        .map_err(|error| {
                            CoreSpaceChangesError::recorded_state_access(
                                format!("load the vote list captured for {account}"),
                                error,
                            )
                        })?;
                let vote_list = VoteStakeList(vote_list);
                verify_vote_list_consistency(&vote_list, account)?;
                entry.insert(vote_list)
            }
        };

        vote_list.remove_expired_vote_stake_info(current_block_number);
        verify_vote_list_consistency(vote_list, account)?;
        let currently_locked_amount = vote_list
            .first()
            .map_or(U256::ZERO, |vote_info| u256_from_cfx(vote_info.amount));
        let withdrawable_staking_balance = staking_balance
            .checked_sub(currently_locked_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space vote-list lock exceeded staking balance for {account}: locked {currently_locked_amount}, staking {staking_balance}"
                ))
            })?;
        if let CommittedStakingCall::Withdrawal {
            principal_amount, ..
        } = *committed_call
        {
            if withdrawable_staking_balance < principal_amount {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space staking withdrawal exceeded vote-lock withdrawable balance for {account}: principal {principal_amount}, withdrawable {withdrawable_staking_balance}"
                )));
            }
            *staking_balance = staking_balance
                .checked_sub(principal_amount)
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space staking balance underflowed at a withdrawal for {account}"
                    ))
                })?;
            continue;
        }
        let CommittedStakingCall::VoteLock {
            position,
            required_locked_amount,
            unlock_block_number,
            ..
        } = *committed_call
        else {
            continue;
        };
        if required_locked_amount > *staking_balance {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space voteLock required more than the call-position staking balance for {account}: required {required_locked_amount}, staking {staking_balance}"
            )));
        }
        if apply_vote_lock_requirement(vote_list, required_locked_amount, unlock_block_number) {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                position,
                PendingCoreSpaceChange::StakingVoteLock {
                    account,
                    required_locked_amount,
                    unlock_block_number,
                },
            ));
        }
    }

    for (account, vote_list) in &vote_lists_by_account {
        verify_post_state_vote_list(post_state, vote_list, *account)?;
    }
    Ok(positioned_changes)
}

fn apply_vote_lock_requirement(
    vote_list: &mut VoteStakeList,
    required_locked_amount: U256,
    unlock_block_number: u64,
) -> bool {
    let vote_list_before = vote_list.clone();
    if !required_locked_amount.is_zero() {
        vote_list.vote_lock(u256_to_cfx(required_locked_amount), unlock_block_number);
    }
    *vote_list != vote_list_before
}

fn verify_post_state_vote_list(
    post_state: &State,
    vote_list: &VoteStakeList,
    account: Address,
) -> Result<(), CoreSpaceChangesError> {
    verify_vote_list_consistency(vote_list, account)?;
    let cfx_account = address_to_cfx(account);
    let post_state_length = post_state
        .vote_stake_list_length(&cfx_account)
        .map_err(|error| post_state_read_error(account, error))?;
    if post_state_length != vote_list.len() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space vote-list length mismatch for {account}: replayed {}, post-state {post_state_length}",
            vote_list.len()
        )));
    }
    for (index, vote_info) in vote_list.iter().enumerate() {
        let unlock_block_number = u64::try_from(vote_info.unlock_block_number).map_err(|_| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote-list unlock block number exceeds u64 for {account}"
            ))
        })?;
        let previous_block = unlock_block_number.checked_sub(1).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote-list unlock block number was zero for {account}"
            ))
        })?;
        let locked_before_unlock = post_state
            .locked_staking_balance_at_block_number(&cfx_account, previous_block)
            .map_err(|error| post_state_read_error(account, error))?;
        if u256_from_cfx(locked_before_unlock) != u256_from_cfx(vote_info.amount) {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote-list locked balance before block {unlock_block_number} mismatched for {account}: expected {}, post-state {}",
                vote_info.amount, locked_before_unlock
            )));
        }
        let locked_at_unlock = post_state
            .locked_staking_balance_at_block_number(&cfx_account, unlock_block_number)
            .map_err(|error| post_state_read_error(account, error))?;
        let required_at_unlock = vote_list
            .get(index + 1)
            .map_or(U256::ZERO, |next| u256_from_cfx(next.amount));
        if u256_from_cfx(locked_at_unlock) != required_at_unlock {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote-list locked balance at block {unlock_block_number} mismatched for {account}: expected {required_at_unlock}, post-state {}",
                u256_from_cfx(locked_at_unlock)
            )));
        }
    }
    Ok(())
}

fn verify_vote_list_consistency(
    vote_list: &VoteStakeList,
    account: Address,
) -> Result<(), CoreSpaceChangesError> {
    for (earlier, later) in vote_list.iter().zip(vote_list.iter().skip(1)) {
        if earlier.unlock_block_number >= later.unlock_block_number
            || earlier.amount <= later.amount
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space vote list is not canonical for {account}"
            )));
        }
    }
    if vote_list.iter().any(|vote_info| vote_info.amount.is_zero()) {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space vote list contains a zero lock amount for {account}"
        )));
    }
    Ok(())
}

fn post_state_read_error(account: Address, error: cfx_statedb::Error) -> CoreSpaceChangesError {
    CoreSpaceChangesError::state_read(
        format!("post-execution vote-list verification for {account}"),
        error,
    )
}

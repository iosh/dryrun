use std::collections::{BTreeMap, btree_map::Entry};

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use primitives::VoteStakeList;

use super::{CommittedStakingCalls, StakingCall};
use crate::{
    ConfluxSimulationError,
    core_space::changes::{CoreSpaceChange, PositionedCoreSpaceChange},
    primitive::{address_to_cfx, u256_from_cfx, u256_to_cfx},
    state::AnchoredVoteLists,
};

pub(crate) fn verify_vote_lock_changes(
    state: &State,
    committed_staking_calls: &CommittedStakingCalls,
    anchored_vote_lists: &AnchoredVoteLists,
    current_block_number: u64,
) -> Result<Vec<PositionedCoreSpaceChange>, ConfluxSimulationError> {
    let mut vote_lists_by_account = BTreeMap::new();
    let mut positioned_changes = Vec::new();

    for committed_call in committed_staking_calls.iter() {
        let StakingCall::VoteLock {
            position,
            account,
            amount,
            unlock_block_number,
        } = committed_call
        else {
            continue;
        };
        let vote_list = match vote_lists_by_account.entry(*account) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let vote_list = anchored_vote_lists
                    .for_account(address_to_cfx(*account))
                    .map_err(|error| ConfluxSimulationError::StateAccess {
                        message: format!(
                            "failed to obtain execution-read anchored vote list for {account}: {error}"
                        ),
                    })?;
                let vote_list = VoteStakeList(vote_list);
                verify_canonical_vote_list(&vote_list, *account)?;
                entry.insert(vote_list)
            }
        };

        vote_list.remove_expired_vote_stake_info(current_block_number);
        verify_canonical_vote_list(vote_list, *account)?;
        let required_locked_raw_amount_before =
            required_locked_amount_before_call(vote_list, *unlock_block_number);
        let vote_list_before_call = vote_list.clone();
        if !amount.is_zero() {
            vote_list.vote_lock(u256_to_cfx(*amount), *unlock_block_number);
        }
        if *vote_list != vote_list_before_call {
            positioned_changes.push(PositionedCoreSpaceChange::new(
                *position,
                CoreSpaceChange::StakingVoteLock {
                    account: *account,
                    unlock_block_number: *unlock_block_number,
                    required_locked_raw_amount_before,
                    required_locked_raw_amount_after: *amount,
                },
            ));
        }
    }

    for (account, vote_list) in &vote_lists_by_account {
        verify_vote_list_after_execution(state, vote_list, *account)?;
    }
    Ok(positioned_changes)
}

fn required_locked_amount_before_call(vote_list: &VoteStakeList, unlock_block_number: u64) -> U256 {
    let unlock_block_number = cfx_types::U256::from(unlock_block_number);
    let index = match vote_list
        .binary_search_by(|vote_info| vote_info.unlock_block_number.cmp(&unlock_block_number))
    {
        Ok(index) | Err(index) => index,
    };
    vote_list
        .get(index)
        .map_or(U256::ZERO, |vote_info| u256_from_cfx(vote_info.amount))
}

fn verify_vote_list_after_execution(
    state: &State,
    vote_list: &VoteStakeList,
    account: Address,
) -> Result<(), ConfluxSimulationError> {
    verify_canonical_vote_list(vote_list, account)?;
    let cfx_account = address_to_cfx(account);
    let actual_length = state
        .vote_stake_list_length(&cfx_account)
        .map_err(|error| after_state_access(account, error))?;
    if actual_length != vote_list.len() {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space vote-list length mismatch for {account}: expected {}, got {actual_length}",
            vote_list.len()
        )));
    }
    for (index, vote_info) in vote_list.iter().enumerate() {
        let unlock_block_number = u64::try_from(vote_info.unlock_block_number).map_err(|_| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core Space vote-list unlock block number exceeds u64 for {account}"
            ))
        })?;
        let previous_block = unlock_block_number - 1;
        let before_unlock = state
            .locked_staking_balance_at_block_number(&cfx_account, previous_block)
            .map_err(|error| after_state_access(account, error))?;
        if u256_from_cfx(before_unlock) != u256_from_cfx(vote_info.amount) {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space vote-list locked balance before boundary mismatched for {account}"
            )));
        }
        let locked_at_unlock = state
            .locked_staking_balance_at_block_number(&cfx_account, unlock_block_number)
            .map_err(|error| after_state_access(account, error))?;
        let required_at_unlock = vote_list
            .get(index + 1)
            .map_or(U256::ZERO, |next| u256_from_cfx(next.amount));
        if u256_from_cfx(locked_at_unlock) != required_at_unlock {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space vote-list locked balance at boundary mismatched for {account}"
            )));
        }
    }
    Ok(())
}

fn verify_canonical_vote_list(
    vote_list: &VoteStakeList,
    account: Address,
) -> Result<(), ConfluxSimulationError> {
    for (earlier, later) in vote_list.iter().zip(vote_list.iter().skip(1)) {
        if earlier.unlock_block_number >= later.unlock_block_number
            || earlier.amount <= later.amount
        {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space vote list is not canonical for {account}"
            )));
        }
    }
    if vote_list.iter().any(|vote_info| vote_info.amount.is_zero()) {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space vote list contains a zero lock amount for {account}"
        )));
    }
    Ok(())
}

fn after_state_access(account: Address, error: cfx_statedb::Error) -> ConfluxSimulationError {
    ConfluxSimulationError::StateAccess {
        message: format!("failed to read after Core Space vote-list state for {account}: {error}"),
    }
}

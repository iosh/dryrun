use std::collections::{BTreeMap, btree_map::Entry};

use alloy_primitives::{Address, U256};
use cfx_executor::state::State;
use primitives::{DepositInfo, DepositList};

use super::CommittedStakingCall;
use crate::{
    core_space::changes::{PendingCoreSpaceChange, PositionedCoreSpaceChange},
    core_space::{CoreSpaceChangesError, changes::cfx::CfxStateValues},
    primitive::{address_to_cfx, u256_from_cfx, u256_to_cfx},
    state::RecordedDepositLists,
};

pub(crate) fn analyze_balance_changes(
    post_state: &State,
    committed_staking_calls: &[CommittedStakingCall],
    deposit_lists: &RecordedDepositLists,
    accumulated_interest_rate: cfx_types::U256,
    current_block_number: u64,
    cip97: bool,
    before_cfx_state: &CfxStateValues,
) -> Result<Vec<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    let mut replays_by_account = BTreeMap::new();
    let mut positioned_changes = Vec::new();

    for committed_call in committed_staking_calls {
        match *committed_call {
            CommittedStakingCall::Deposit {
                position,
                account,
                amount,
                ..
            } => {
                if amount.is_zero() {
                    continue;
                }
                let replay = replay_for_account(
                    &mut replays_by_account,
                    account,
                    deposit_lists,
                    before_cfx_state,
                )?;
                replay.deposit(
                    amount,
                    accumulated_interest_rate,
                    current_block_number,
                    cip97,
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    position,
                    PendingCoreSpaceChange::StakingDeposit { account, amount },
                ));
            }
            CommittedStakingCall::Withdrawal {
                position,
                account,
                principal_amount,
                reward_amount,
                ..
            } => {
                if principal_amount.is_zero() {
                    if !reward_amount.is_zero() {
                        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                            "Core Space zero-principal staking withdrawal issued a nonzero reward for {account}"
                        )));
                    }
                    continue;
                }
                let replay = replay_for_account(
                    &mut replays_by_account,
                    account,
                    deposit_lists,
                    before_cfx_state,
                )?;
                let replayed_reward =
                    replay.withdraw(principal_amount, accumulated_interest_rate, cip97)?;
                if replayed_reward != reward_amount {
                    return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space staking withdrawal reward mismatch for {account}: replayed {replayed_reward}, traced {reward_amount}"
                    )));
                }
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    position,
                    PendingCoreSpaceChange::StakingWithdrawal {
                        account,
                        principal_amount,
                        reward_amount,
                    },
                ));
            }
            CommittedStakingCall::VoteLock { .. } => {}
        }
    }

    for (account, replay) in &replays_by_account {
        verify_deposit_list_consistency(&replay.deposit_list, replay.staking_balance, *account)?;
        let post_state_length = post_state
            .deposit_list_length(&address_to_cfx(*account))
            .map_err(|error| {
                CoreSpaceChangesError::state_read(
                    format!("post-execution deposit-list length lookup for {account}"),
                    error,
                )
            })?;
        if post_state_length != replay.deposit_list.len() {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space deposit-list length mismatch for {account}: replayed {}, post-state {post_state_length}",
                replay.deposit_list.len()
            )));
        }
    }

    Ok(positioned_changes)
}

fn replay_for_account<'a>(
    replays_by_account: &'a mut BTreeMap<Address, StakingAccountReplay>,
    account: Address,
    deposit_lists: &RecordedDepositLists,
    before_cfx_state: &CfxStateValues,
) -> Result<&'a mut StakingAccountReplay, CoreSpaceChangesError> {
    match replays_by_account.entry(account) {
        Entry::Occupied(entry) => Ok(entry.into_mut()),
        Entry::Vacant(entry) => {
            let deposit_list =
                deposit_lists
                    .for_account(address_to_cfx(account))
                    .map_err(|error| {
                        CoreSpaceChangesError::recorded_state_access(
                            format!("load the deposit list captured for {account}"),
                            error,
                        )
                    })?;
            let staking_balance = before_cfx_state.staking_balance(account)?;
            verify_deposit_list_consistency(&deposit_list, staking_balance, account)?;
            Ok(entry.insert(StakingAccountReplay {
                account,
                staking_balance,
                deposit_list: DepositList(deposit_list),
            }))
        }
    }
}

struct StakingAccountReplay {
    account: Address,
    staking_balance: U256,
    deposit_list: DepositList,
}

impl StakingAccountReplay {
    fn deposit(
        &mut self,
        amount: U256,
        accumulated_interest_rate: cfx_types::U256,
        current_block_number: u64,
        cip97: bool,
    ) -> Result<(), CoreSpaceChangesError> {
        self.staking_balance = self.staking_balance.checked_add(amount).ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space staking balance overflowed while replaying a deposit for {}",
                self.account
            ))
        })?;
        if !(cip97 && self.deposit_list.is_empty()) {
            self.deposit_list.push(DepositInfo {
                amount: u256_to_cfx(amount),
                deposit_time: current_block_number.into(),
                accumulated_interest_rate,
            });
        }
        Ok(())
    }

    fn withdraw(
        &mut self,
        principal_amount: U256,
        accumulated_interest_rate: cfx_types::U256,
        cip97: bool,
    ) -> Result<U256, CoreSpaceChangesError> {
        if principal_amount.is_zero() {
            return Ok(U256::ZERO);
        }
        let before_staking_balance = self.staking_balance;
        self.staking_balance = self
            .staking_balance
            .checked_sub(principal_amount)
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space staking balance underflowed while replaying a withdrawal for {}",
                    self.account
                ))
            })?;
        if self.deposit_list.is_empty() {
            return Ok(U256::ZERO);
        }

        let mut remaining_principal = if cip97 {
            before_staking_balance
        } else {
            principal_amount
        };
        let mut reward = U256::ZERO;
        let mut consumed_entries = 0;
        while !remaining_principal.is_zero() {
            let Some(deposit_entry) = self.deposit_list.get_mut(consumed_entries) else {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space deposit list did not cover staking withdrawal for {}",
                    self.account
                )));
            };
            let deposit_amount = u256_from_cfx(deposit_entry.amount);
            let capital = deposit_amount.min(remaining_principal);
            let deposit_rate = u256_from_cfx(deposit_entry.accumulated_interest_rate);
            if deposit_rate.is_zero() {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space deposit list contained a zero interest rate for {}",
                    self.account
                )));
            }
            if accumulated_interest_rate < deposit_entry.accumulated_interest_rate {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space accumulated interest rate went backwards for {}: current {accumulated_interest_rate}, deposit {}",
                    self.account, deposit_entry.accumulated_interest_rate
                )));
            }
            let entry_reward = capital
                .checked_mul(u256_from_cfx(accumulated_interest_rate))
                .and_then(|value| value.checked_div(deposit_rate))
                .and_then(|value| value.checked_sub(capital))
                .ok_or_else(|| {
                    CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space staking interest arithmetic failed for {}",
                        self.account
                    ))
                })?;
            reward = reward.checked_add(entry_reward).ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space staking interest overflowed for {}",
                    self.account
                ))
            })?;
            deposit_entry.amount = u256_to_cfx(deposit_amount - capital);
            remaining_principal -= capital;
            if deposit_entry.amount.is_zero() {
                consumed_entries += 1;
            }
        }
        if consumed_entries > 0 {
            self.deposit_list.0.drain(..consumed_entries);
        }
        Ok(reward)
    }
}

fn verify_deposit_list_consistency(
    deposit_list: &[DepositInfo],
    staking_balance: U256,
    account: Address,
) -> Result<(), CoreSpaceChangesError> {
    if deposit_list
        .iter()
        .any(|deposit| deposit.accumulated_interest_rate.is_zero())
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space deposit list contains a zero interest rate for {account}"
        )));
    }
    if !deposit_list.is_empty() {
        let listed_staking_balance = deposit_list
            .iter()
            .try_fold(U256::ZERO, |total, deposit| {
                total.checked_add(u256_from_cfx(deposit.amount))
            })
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space deposit-list principal overflowed for {account}"
                ))
            })?;
        if listed_staking_balance != staking_balance {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space deposit-list principal did not match staking balance for {account}: listed {listed_staking_balance}, staking {staking_balance}"
            )));
        }
    }
    Ok(())
}

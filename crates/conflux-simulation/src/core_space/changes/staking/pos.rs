use alloy_primitives::{Address, B256, Bytes, Keccak256, U256};

use super::{
    CommittedPoSCall,
    codec::{PoSEvent, decode_pos_events},
    pos_state::{
        PoSStateRequirements, PoSStateValues, account_for_identifier, identifier_for_account,
        pos_status, verify_mapping,
    },
};
use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::{
        PendingCoreSpaceChange, PositionedCoreSpaceChange, cfx::StakingBalanceEffects,
    },
    primitive::u256_from_cfx,
};
use primitives::LogEntry;

#[derive(Debug)]
pub(crate) struct PoSAnalysisInput {
    calls: Vec<CommittedPoSCall>,
    events: Vec<PoSEvent>,
    state_requirements: PoSStateRequirements,
}

impl PoSAnalysisInput {
    pub(crate) fn from_calls_and_logs(
        calls: &[CommittedPoSCall],
        final_logs: &[LogEntry],
        pos_register_contract_active: bool,
    ) -> Result<Self, CoreSpaceChangesError> {
        Ok(Self {
            calls: calls.to_vec(),
            events: decode_pos_events(final_logs, pos_register_contract_active)?,
            state_requirements: PoSStateRequirements::from_pos_calls(calls),
        })
    }

    pub(crate) fn calls(&self) -> &[CommittedPoSCall] {
        &self.calls
    }

    pub(crate) fn events(&self) -> &[PoSEvent] {
        &self.events
    }

    pub(crate) fn state_requirements(&self) -> &PoSStateRequirements {
        &self.state_requirements
    }
}

#[derive(Clone, Copy)]
enum StakeIncreaseKind {
    Registration,
    Increase,
}

pub(crate) fn analyze_pos_changes(
    pos_analysis: &PoSAnalysisInput,
    before_state: &PoSStateValues,
    after_state: &PoSStateValues,
    staking_balance_effects: &StakingBalanceEffects,
) -> Result<Vec<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    let mut replayed_after_state = before_state.clone();
    let mut remaining_pos_events = pos_analysis.events().iter();
    let mut positioned_changes = Vec::new();

    for committed_call in pos_analysis.calls() {
        match committed_call {
            CommittedPoSCall::Registration {
                position,
                account,
                identifier,
                vote_count,
            } => {
                let (bls_public_key, vrf_public_key) =
                    consume_registration_event(&mut remaining_pos_events, *identifier)?;
                verify_registration_identifier(*identifier, &bls_public_key, &vrf_public_key)?;
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::IncreaseStake {
                        identifier: *identifier,
                        vote_count: *vote_count,
                    },
                )?;

                replay_registration(&mut replayed_after_state, *account, *identifier)?;
                let locked_amount = replay_stake_increase(
                    &mut replayed_after_state,
                    *identifier,
                    *account,
                    *vote_count,
                    StakeIncreaseKind::Registration,
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    PendingCoreSpaceChange::PoSRegistration {
                        account: *account,
                        identifier: *identifier,
                        bls_public_key,
                        vrf_public_key,
                        initial_vote_count: *vote_count,
                        locked_amount,
                    },
                ));
            }
            CommittedPoSCall::StakeIncrease {
                position,
                account,
                vote_count,
            } => {
                let identifier =
                    registered_identifier(&replayed_after_state, *account, "increase")?;
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::IncreaseStake {
                        identifier,
                        vote_count: *vote_count,
                    },
                )?;
                let added_locked_amount = replay_stake_increase(
                    &mut replayed_after_state,
                    identifier,
                    *account,
                    *vote_count,
                    StakeIncreaseKind::Increase,
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    PendingCoreSpaceChange::PoSStakeIncrease {
                        account: *account,
                        identifier,
                        added_vote_count: *vote_count,
                        added_locked_amount,
                    },
                ));
            }
            CommittedPoSCall::RetirementRequest {
                position,
                account,
                requested_vote_count,
            } => {
                let identifier =
                    registered_identifier(&replayed_after_state, *account, "retire request")?;
                if !pos_status(&replayed_after_state, identifier)?.has_locked_votes() {
                    return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                        "Core Space PoS retire request targeted an unlocked identifier for {account}"
                    )));
                }
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::Retire {
                        identifier,
                        requested_vote_count: *requested_vote_count,
                    },
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    PendingCoreSpaceChange::PoSRetirementRequest {
                        account: *account,
                        identifier,
                        requested_vote_count: *requested_vote_count,
                    },
                ));
            }
        }
    }

    if remaining_pos_events.next().is_some() {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS event replay left unmatched final logs",
        ));
    }
    verify_replay_matches_after_state(&replayed_after_state, after_state)?;
    verify_staking_balance_replay(before_state, after_state, staking_balance_effects)?;
    verify_staking_coverage(after_state)?;
    Ok(positioned_changes)
}

fn replay_registration(
    replayed_after_state: &mut PoSStateValues,
    account: Address,
    identifier: B256,
) -> Result<(), CoreSpaceChangesError> {
    let existing_identifier = identifier_for_account(replayed_after_state, account)?;
    if !existing_identifier.is_zero() {
        verify_mapping(replayed_after_state, existing_identifier, account)?;
        let existing_status = pos_status(replayed_after_state, existing_identifier)?;
        if !existing_status.is_fully_unlocked() {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS registration changed a still-locked identifier for {account}"
            )));
        }
    }
    if !account_for_identifier(replayed_after_state, identifier)?.is_zero() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space PoS registration reused identifier {identifier}"
        )));
    }
    replayed_after_state
        .identifiers_by_account
        .insert(account, identifier);
    replayed_after_state
        .accounts_by_identifier
        .insert(identifier, account);
    Ok(())
}

fn registered_identifier(
    replayed_after_state: &PoSStateValues,
    account: Address,
    action: &str,
) -> Result<B256, CoreSpaceChangesError> {
    let identifier = identifier_for_account(replayed_after_state, account)?;
    if identifier.is_zero() {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space PoS {action} has no registered identifier for {account}"
        )));
    }
    verify_mapping(replayed_after_state, identifier, account)?;
    Ok(identifier)
}

fn replay_stake_increase(
    replayed_after_state: &mut PoSStateValues,
    identifier: B256,
    account: Address,
    vote_count: u64,
    kind: StakeIncreaseKind,
) -> Result<U256, CoreSpaceChangesError> {
    if vote_count == 0 {
        return Err(CoreSpaceChangesError::inconsistent_execution(format!(
            "Core Space PoS increase used zero votes for {account}"
        )));
    }
    verify_mapping(replayed_after_state, identifier, account)?;
    let status = replayed_after_state
        .statuses_by_identifier
        .get_mut(&identifier)
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS increase did not have a status for {identifier}"
            ))
        })?;
    match kind {
        StakeIncreaseKind::Registration if status.is_initialized() => {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS registration initialized an existing identifier {identifier}"
            )));
        }
        StakeIncreaseKind::Increase if !status.is_initialized() => {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS increase used an uninitialized identifier {identifier}"
            )));
        }
        StakeIncreaseKind::Registration | StakeIncreaseKind::Increase => {}
    }
    status
        .checked_add_registered_votes(vote_count)
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS registered vote count overflowed for {identifier}"
            ))
        })?;
    let locked_amount = vote_amount(vote_count)?;
    replayed_after_state.total_pos_staking = replayed_after_state
        .total_pos_staking
        .checked_add(locked_amount)
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution(
                "Core Space total PoS staking overflowed during an increase",
            )
        })?;
    Ok(locked_amount)
}

fn vote_amount(vote_count: u64) -> Result<U256, CoreSpaceChangesError> {
    U256::from(vote_count)
        .checked_mul(u256_from_cfx(*cfx_parameters::staking::POS_VOTE_PRICE))
        .ok_or_else(|| {
            CoreSpaceChangesError::inconsistent_execution("Core Space PoS vote value overflowed")
        })
}

fn consume_registration_event<'event>(
    pos_events: &mut impl Iterator<Item = &'event PoSEvent>,
    expected_identifier: B256,
) -> Result<(Bytes, Bytes), CoreSpaceChangesError> {
    let Some(PoSEvent::Register {
        identifier,
        bls_public_key,
        vrf_public_key,
    }) = pos_events.next()
    else {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS registration did not have the expected Register event",
        ));
    };
    if *identifier != expected_identifier {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS Register event identifier does not match the positioned call",
        ));
    }
    Ok((bls_public_key.clone(), vrf_public_key.clone()))
}

fn verify_registration_identifier(
    identifier: B256,
    bls_public_key: &Bytes,
    vrf_public_key: &Bytes,
) -> Result<(), CoreSpaceChangesError> {
    let mut hasher = Keccak256::new();
    hasher.update(bls_public_key);
    hasher.update(vrf_public_key);
    if hasher.finalize() != identifier {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS registration public keys do not match the identifier",
        ));
    }
    Ok(())
}

fn consume_pos_event<'event>(
    pos_events: &mut impl Iterator<Item = &'event PoSEvent>,
    expected_event: PoSEvent,
) -> Result<(), CoreSpaceChangesError> {
    if pos_events.next() != Some(&expected_event) {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS final log does not match the positioned call replay",
        ));
    }
    Ok(())
}

fn verify_replay_matches_after_state(
    replayed_after_state: &PoSStateValues,
    after_state: &PoSStateValues,
) -> Result<(), CoreSpaceChangesError> {
    if replayed_after_state.identifiers_by_account != after_state.identifiers_by_account
        || replayed_after_state.accounts_by_identifier != after_state.accounts_by_identifier
        || replayed_after_state.statuses_by_identifier != after_state.statuses_by_identifier
        || replayed_after_state.total_pos_staking != after_state.total_pos_staking
    {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS storage replay does not match after state",
        ));
    }
    for (account, identifier) in &after_state.identifiers_by_account {
        if !identifier.is_zero() {
            verify_mapping(after_state, *identifier, *account)?;
        }
    }
    Ok(())
}

fn verify_staking_balance_replay(
    before_state: &PoSStateValues,
    after_state: &PoSStateValues,
    staking_balance_effects: &StakingBalanceEffects,
) -> Result<(), CoreSpaceChangesError> {
    let mut replayed_staking_balances = before_state.staking_balances_by_account.clone();
    staking_balance_effects.apply_to(&mut replayed_staking_balances)?;
    if replayed_staking_balances != after_state.staking_balances_by_account {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space PoS staking balances changed beyond verified CFX staking movements",
        ));
    }
    Ok(())
}

fn verify_staking_coverage(after_state: &PoSStateValues) -> Result<(), CoreSpaceChangesError> {
    for (account, identifier) in &after_state.identifiers_by_account {
        if identifier.is_zero() {
            continue;
        }
        let required_staking =
            vote_amount(pos_status(after_state, *identifier)?.locked_vote_count())?;
        let staking_balance = after_state
            .staking_balances_by_account
            .get(account)
            .copied()
            .ok_or_else(|| {
                CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space PoS after state did not include staking balance for {account}"
                ))
            })?;
        if staking_balance < required_staking {
            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                "Core Space PoS staking balance cannot cover locked votes for {account}"
            )));
        }
    }
    Ok(())
}

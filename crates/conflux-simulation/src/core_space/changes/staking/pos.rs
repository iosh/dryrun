use alloy_primitives::{Address, B256, U256};

use super::{
    CommittedPoSCall,
    codec::{PoSEvent, decode_pos_staking_events},
    pos_state::{
        PoSStateRequirements, PoSStateValues, pos_identifier_account, pos_status,
        sender_pos_identifier, verify_pos_identifier_account_pair,
    },
};
use crate::{
    ConfluxSimulationError,
    core_space::changes::{CoreSpaceChange, PositionedCoreSpaceChange, cfx::StakingBalanceEffects},
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
    ) -> Result<Self, ConfluxSimulationError> {
        Ok(Self {
            calls: calls.to_vec(),
            events: decode_pos_staking_events(final_logs, pos_register_contract_active)?,
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
enum PoSStakeIncreaseSource {
    RegistrationCall,
    IncreaseStakeCall,
}

pub(crate) fn verify_pos_staking_changes(
    pos_analysis: &PoSAnalysisInput,
    before_state: &PoSStateValues,
    after_state: &PoSStateValues,
    staking_balance_effects: &StakingBalanceEffects,
) -> Result<Vec<PositionedCoreSpaceChange>, ConfluxSimulationError> {
    let mut replayed_after_state = before_state.clone();
    let mut remaining_pos_events = pos_analysis.events().iter();
    let mut positioned_changes = Vec::new();

    for committed_call in pos_analysis.calls() {
        match committed_call {
            CommittedPoSCall::Registration {
                position,
                account,
                pos_identifier,
                vote_count,
            } => {
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::Register {
                        pos_identifier: *pos_identifier,
                    },
                )?;
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::IncreaseStake {
                        pos_identifier: *pos_identifier,
                        vote_count: *vote_count,
                    },
                )?;

                replay_registration(&mut replayed_after_state, *account, *pos_identifier)?;
                let newly_locked_raw_amount = replay_stake_increase(
                    &mut replayed_after_state,
                    *pos_identifier,
                    *account,
                    *vote_count,
                    PoSStakeIncreaseSource::RegistrationCall,
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::PoSRegistration {
                        account: *account,
                        pos_identifier: *pos_identifier,
                        newly_locked_vote_count: *vote_count,
                        newly_locked_raw_amount,
                    },
                ));
            }
            CommittedPoSCall::StakeIncrease {
                position,
                account,
                vote_count,
            } => {
                let pos_identifier =
                    registered_pos_identifier(&replayed_after_state, *account, "increase")?;
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::IncreaseStake {
                        pos_identifier,
                        vote_count: *vote_count,
                    },
                )?;
                let newly_locked_raw_amount = replay_stake_increase(
                    &mut replayed_after_state,
                    pos_identifier,
                    *account,
                    *vote_count,
                    PoSStakeIncreaseSource::IncreaseStakeCall,
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::PoSStakeIncrease {
                        account: *account,
                        pos_identifier,
                        newly_locked_vote_count: *vote_count,
                        newly_locked_raw_amount,
                    },
                ));
            }
            CommittedPoSCall::RetirementRequest {
                position,
                account,
                requested_vote_count,
            } => {
                let pos_identifier =
                    registered_pos_identifier(&replayed_after_state, *account, "retire request")?;
                if !pos_status(&replayed_after_state, pos_identifier)?.has_locked_votes() {
                    return Err(ConfluxSimulationError::analysis_failed(format!(
                        "Core Space PoS retire request targeted an unlocked identifier for {account}"
                    )));
                }
                consume_pos_event(
                    &mut remaining_pos_events,
                    PoSEvent::Retire {
                        pos_identifier,
                        requested_vote_count: *requested_vote_count,
                    },
                )?;
                positioned_changes.push(PositionedCoreSpaceChange::new(
                    *position,
                    CoreSpaceChange::PoSRetirementRequest {
                        account: *account,
                        pos_identifier,
                        requested_vote_count: *requested_vote_count,
                    },
                ));
            }
        }
    }

    if remaining_pos_events.next().is_some() {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS event replay left unmatched final logs",
        ));
    }
    verify_replay_matches_after_state(&replayed_after_state, after_state)?;
    verify_staking_balance_replay(before_state, after_state, staking_balance_effects)?;
    verify_after_staking_coverage(after_state)?;
    Ok(positioned_changes)
}

fn replay_registration(
    replayed_after_state: &mut PoSStateValues,
    account: Address,
    pos_identifier: B256,
) -> Result<(), ConfluxSimulationError> {
    let existing_pos_identifier = sender_pos_identifier(replayed_after_state, account)?;
    if !existing_pos_identifier.is_zero() {
        verify_pos_identifier_account_pair(replayed_after_state, existing_pos_identifier, account)?;
        let existing_status = pos_status(replayed_after_state, existing_pos_identifier)?;
        if !existing_status.is_fully_unlocked() {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS registration changed a still-locked identifier for {account}"
            )));
        }
    }
    if !pos_identifier_account(replayed_after_state, pos_identifier)?.is_zero() {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space PoS registration reused identifier {pos_identifier}"
        )));
    }
    replayed_after_state
        .sender_pos_identifiers
        .insert(account, pos_identifier);
    replayed_after_state
        .pos_identifier_accounts
        .insert(pos_identifier, account);
    Ok(())
}

fn registered_pos_identifier(
    replayed_after_state: &PoSStateValues,
    account: Address,
    action: &str,
) -> Result<B256, ConfluxSimulationError> {
    let pos_identifier = sender_pos_identifier(replayed_after_state, account)?;
    if pos_identifier.is_zero() {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space PoS {action} has no registered identifier for {account}"
        )));
    }
    verify_pos_identifier_account_pair(replayed_after_state, pos_identifier, account)?;
    Ok(pos_identifier)
}

fn replay_stake_increase(
    replayed_after_state: &mut PoSStateValues,
    pos_identifier: B256,
    account: Address,
    vote_count: u64,
    source: PoSStakeIncreaseSource,
) -> Result<U256, ConfluxSimulationError> {
    if vote_count == 0 {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space PoS increase used zero votes for {account}"
        )));
    }
    verify_pos_identifier_account_pair(replayed_after_state, pos_identifier, account)?;
    let status = replayed_after_state
        .pos_statuses
        .get_mut(&pos_identifier)
        .ok_or_else(|| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS increase did not have a status for {pos_identifier}"
            ))
        })?;
    match source {
        PoSStakeIncreaseSource::RegistrationCall if status.is_initialized() => {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS registration initialized an existing identifier {pos_identifier}"
            )));
        }
        PoSStakeIncreaseSource::IncreaseStakeCall if !status.is_initialized() => {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS increase used an uninitialized identifier {pos_identifier}"
            )));
        }
        PoSStakeIncreaseSource::RegistrationCall | PoSStakeIncreaseSource::IncreaseStakeCall => {}
    }
    status
        .checked_add_registered_votes(vote_count)
        .ok_or_else(|| {
            ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS registered vote count overflowed for {pos_identifier}"
            ))
        })?;
    let newly_locked_raw_amount = raw_vote_amount(vote_count)?;
    replayed_after_state.total_pos_staking = replayed_after_state
        .total_pos_staking
        .checked_add(newly_locked_raw_amount)
        .ok_or_else(|| {
            ConfluxSimulationError::analysis_failed(
                "Core Space total PoS staking overflowed during an increase",
            )
        })?;
    Ok(newly_locked_raw_amount)
}

fn raw_vote_amount(vote_count: u64) -> Result<U256, ConfluxSimulationError> {
    U256::from(vote_count)
        .checked_mul(u256_from_cfx(*cfx_parameters::staking::POS_VOTE_PRICE))
        .ok_or_else(|| {
            ConfluxSimulationError::analysis_failed("Core Space PoS vote value overflowed")
        })
}

fn consume_pos_event<'event>(
    pos_events: &mut impl Iterator<Item = &'event PoSEvent>,
    expected_event: PoSEvent,
) -> Result<(), ConfluxSimulationError> {
    if pos_events.next() != Some(&expected_event) {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS final log does not match the positioned call replay",
        ));
    }
    Ok(())
}

fn verify_replay_matches_after_state(
    replayed_after_state: &PoSStateValues,
    after_state: &PoSStateValues,
) -> Result<(), ConfluxSimulationError> {
    if replayed_after_state.sender_pos_identifiers != after_state.sender_pos_identifiers
        || replayed_after_state.pos_identifier_accounts != after_state.pos_identifier_accounts
        || replayed_after_state.pos_statuses != after_state.pos_statuses
        || replayed_after_state.total_pos_staking != after_state.total_pos_staking
    {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS storage replay does not match after state",
        ));
    }
    for (account, pos_identifier) in &after_state.sender_pos_identifiers {
        if !pos_identifier.is_zero() {
            verify_pos_identifier_account_pair(after_state, *pos_identifier, *account)?;
        }
    }
    Ok(())
}

fn verify_staking_balance_replay(
    before_state: &PoSStateValues,
    after_state: &PoSStateValues,
    staking_balance_effects: &StakingBalanceEffects,
) -> Result<(), ConfluxSimulationError> {
    let mut replayed_staking_balances = before_state.staking_balances.clone();
    staking_balance_effects.apply_to(&mut replayed_staking_balances)?;
    if replayed_staking_balances != after_state.staking_balances {
        return Err(ConfluxSimulationError::analysis_failed(
            "Core Space PoS staking balances changed beyond verified CFX staking movements",
        ));
    }
    Ok(())
}

fn verify_after_staking_coverage(
    after_state: &PoSStateValues,
) -> Result<(), ConfluxSimulationError> {
    for (account, pos_identifier) in &after_state.sender_pos_identifiers {
        if pos_identifier.is_zero() {
            continue;
        }
        let required_staking =
            raw_vote_amount(pos_status(after_state, *pos_identifier)?.locked_vote_count())?;
        let staking_balance = after_state
            .staking_balances
            .get(account)
            .copied()
            .ok_or_else(|| {
                ConfluxSimulationError::analysis_failed(format!(
                    "Core Space PoS after state did not include staking balance for {account}"
                ))
            })?;
        if staking_balance < required_staking {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space PoS staking balance cannot cover locked votes for {account}"
            )));
        }
    }
    Ok(())
}

use super::{GovernanceAnalysisInput, VoteEvent, VoteLogs};
use crate::core_space::{
    CoreSpaceChangesError, GovernanceParameter, GovernanceVote,
    changes::{PendingCoreSpaceChange, PositionedCoreSpaceChange},
};

pub(crate) fn analyze_governance_changes(
    input: &GovernanceAnalysisInput,
) -> Result<Vec<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    let mut changes = Vec::new();
    for logs in input.groups() {
        if let Some(change) = governance_change(logs)? {
            changes.push(change);
        }
    }
    Ok(changes)
}

fn governance_change(
    logs: &VoteLogs,
) -> Result<Option<PositionedCoreSpaceChange>, CoreSpaceChangesError> {
    let Some(first) = logs.events.first() else {
        return Ok(None);
    };
    let round = first.round();
    let voter = first.voter();
    let votes = match first {
        VoteEvent::Vote { .. } => new_round_votes(logs, round, voter)?,
        VoteEvent::Revoke { .. } => replacement_votes(logs, round, voter)?,
    };
    Ok(Some(PositionedCoreSpaceChange::new(
        logs.position,
        PendingCoreSpaceChange::GovernanceVoteCast {
            voter,
            round,
            votes,
        },
    )))
}

fn new_round_votes(
    logs: &VoteLogs,
    round: u64,
    voter: alloy_primitives::Address,
) -> Result<Vec<GovernanceVote>, CoreSpaceChangesError> {
    let mut votes = Vec::with_capacity(logs.events.len());

    for event in &logs.events {
        let VoteEvent::Vote {
            round: event_round,
            voter: event_voter,
            parameter,
            allocation,
        } = *event
        else {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space new-round governance logs mixed Vote and Revoke events",
            ));
        };
        verify_context(round, voter, event_round, event_voter)?;
        votes.push(GovernanceVote {
            parameter: governance_parameter(parameter)?,
            allocation,
            replaced_allocation: None,
        });
    }

    Ok(votes)
}

fn replacement_votes(
    logs: &VoteLogs,
    round: u64,
    voter: alloy_primitives::Address,
) -> Result<Vec<GovernanceVote>, CoreSpaceChangesError> {
    let mut events = logs.events.chunks_exact(2);
    let mut votes = Vec::with_capacity(logs.events.len() / 2);

    for pair in &mut events {
        let VoteEvent::Revoke {
            round: revoke_round,
            voter: revoke_voter,
            parameter: revoke_parameter,
            allocation: replaced_allocation,
        } = pair[0]
        else {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space same-round governance logs did not begin a parameter update with Revoke",
            ));
        };
        let VoteEvent::Vote {
            round: vote_round,
            voter: vote_voter,
            parameter: vote_parameter,
            allocation,
        } = pair[1]
        else {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space same-round governance logs did not finish a parameter update with Vote",
            ));
        };
        verify_context(round, voter, revoke_round, revoke_voter)?;
        verify_context(round, voter, vote_round, vote_voter)?;
        if revoke_parameter != vote_parameter {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "Core Space same-round Revoke and Vote events used different parameters",
            ));
        }
        votes.push(GovernanceVote {
            parameter: governance_parameter(vote_parameter)?,
            allocation,
            replaced_allocation: Some(replaced_allocation),
        });
    }
    if !events.remainder().is_empty() {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space same-round governance logs had an incomplete Revoke and Vote pair",
        ));
    }

    Ok(votes)
}

fn verify_context(
    expected_round: u64,
    expected_voter: alloy_primitives::Address,
    event_round: u64,
    event_voter: alloy_primitives::Address,
) -> Result<(), CoreSpaceChangesError> {
    if event_voter != expected_voter {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space governance events in one frame used different voters",
        ));
    }
    if event_round != expected_round {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "Core Space governance events in one frame used different rounds",
        ));
    }
    Ok(())
}

fn governance_parameter(parameter: u16) -> Result<GovernanceParameter, CoreSpaceChangesError> {
    match parameter {
        0 => Ok(GovernanceParameter::PowBaseReward),
        1 => Ok(GovernanceParameter::PosRewardInterestRate),
        2 => Ok(GovernanceParameter::StoragePointProportion),
        3 => Ok(GovernanceParameter::BaseFeeShareProportion),
        _ => Err(CoreSpaceChangesError::unsupported_operation(format!(
            "Core Space governance event used unknown parameter {parameter}"
        ))),
    }
}

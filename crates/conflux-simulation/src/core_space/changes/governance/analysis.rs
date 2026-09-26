use std::collections::BTreeMap;

use cfx_types::Address;

use super::{
    VoteEvent,
    collection::{GovernanceOperation, collect_operations},
};
use crate::core_space::{
    CoreSpaceChangeSet, CoreSpaceChangeSetBuilder, CoreSpaceExecutedTransaction,
    CoreSpaceProtocolError, CoreSpaceStateAccess, GovernanceParameter, GovernanceVote,
    VoteAllocation,
};

pub(crate) fn derive_changes(
    execution: &CoreSpaceExecutedTransaction,
    state: &CoreSpaceStateAccess,
) -> Result<CoreSpaceChangeSet, CoreSpaceProtocolError> {
    let params = cfx_parameters::internal_contract_addresses::PARAMS_CONTROL_CONTRACT_ADDRESS;
    let operations = collect_operations(
        execution.trace(),
        execution.is_active_internal_contract(params),
    )?;
    if operations.is_empty() {
        return Ok(CoreSpaceChangeSet::default());
    }

    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for operation in &operations {
        before
            .entry(operation.voter)
            .or_insert_with(|| state.initial().governance_state(operation.voter));
        after
            .entry(operation.voter)
            .or_insert_with(|| state.finalized().governance_state(operation.voter));
    }
    let before = before
        .into_iter()
        .map(|(voter, state)| state.map(|state| (voter, state)))
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(state_error)?;
    let after = after
        .into_iter()
        .map(|(voter, state)| state.map(|state| (voter, state)))
        .collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(state_error)?;

    let first_state = after
        .values()
        .next()
        .ok_or_else(|| inconsistent("Core Space governance state is empty"))?;
    let current_round = first_state.current_round;
    if before
        .values()
        .any(|state| state.current_round != current_round)
        || after
            .values()
            .any(|state| state.current_round != current_round)
    {
        return Err(inconsistent(
            "Core Space governance state returned inconsistent rounds",
        ));
    }
    let parameter_count = first_state.parameter_count;
    if before
        .values()
        .chain(after.values())
        .any(|state| state.parameter_count != parameter_count)
    {
        return Err(inconsistent(
            "Core Space governance state returned inconsistent parameter ranges",
        ));
    }

    let mut replayed = before
        .iter()
        .map(|(voter, state)| {
            let allocation = if state.version == current_round {
                state.allocations
            } else {
                [VoteAllocation::default(); 4]
            };
            (*voter, allocation)
        })
        .collect::<BTreeMap<_, _>>();
    let mut builder = CoreSpaceChangeSetBuilder::new();

    for operation in operations {
        if operation.round != current_round {
            return Err(inconsistent(
                "Core Space governance operation used a stale voting round",
            ));
        }
        let Some(allocation) = replayed.get_mut(&operation.voter) else {
            return Err(inconsistent(
                "Core Space governance replay is missing an operation voter",
            ));
        };
        verify_events(&operation, *allocation, parameter_count)?;

        let votes = operation
            .votes
            .iter()
            .map(|(index, value)| {
                let parameter = parameter(*index)?;
                let slot = allocation.get_mut(usize::from(*index)).ok_or_else(|| {
                    inconsistent("Core Space governance parameter index is out of range")
                })?;
                let replaced_allocation = operation.events.iter().find_map(|event| match event {
                    VoteEvent::Revoke {
                        parameter,
                        allocation,
                        ..
                    } if *parameter == *index => Some(*allocation),
                    _ => None,
                });
                *slot = *value;
                Ok(GovernanceVote {
                    parameter,
                    allocation: *value,
                    replaced_allocation,
                })
            })
            .collect::<Result<Vec<_>, CoreSpaceProtocolError>>()?;
        builder.governance_vote(
            operation.position,
            core_address(operation.voter, execution)?,
            operation.round,
            votes,
        );
    }

    for (voter, state) in after {
        if state.version != current_round {
            return Err(inconsistent(
                "Core Space governance final vote version is stale",
            ));
        }
        if replayed[&voter] != state.allocations {
            return Err(inconsistent(
                "Core Space governance replay does not match finalized state",
            ));
        }
    }
    Ok(builder.finish())
}

fn verify_events(
    operation: &GovernanceOperation,
    previous: [VoteAllocation; 4],
    parameter_count: usize,
) -> Result<(), CoreSpaceProtocolError> {
    let expected_voter = alloy_primitives::Address::from_slice(operation.voter.as_bytes());
    if operation.events.is_empty() && operation.votes.is_empty() {
        // A same-round castVote with an empty input is a committed no-op.
        // A stale-version first vote is still caught by the final version and
        // allocation replay below.
        return Ok(());
    }
    if operation.events.is_empty() {
        return Err(inconsistent(
            "Core Space governance call did not emit committed vote events",
        ));
    }
    let replacement = matches!(operation.events[0], VoteEvent::Revoke { .. });
    let mut submitted = [false; 4];
    for (index, _) in &operation.votes {
        let index = usize::from(*index);
        if index >= parameter_count || index >= submitted.len() || submitted[index] {
            return Err(inconsistent(
                "Core Space governance call used an invalid or duplicate parameter",
            ));
        }
        submitted[index] = true;
    }
    if replacement {
        if operation.events.len() != operation.votes.len() * 2 {
            return Err(inconsistent(
                "Core Space governance replacement events do not match submitted votes",
            ));
        }
        let mut events_seen = [false; 4];
        for pair in operation.events.as_slice().as_chunks::<2>().0 {
            let (
                VoteEvent::Revoke {
                    round,
                    voter,
                    parameter,
                    allocation: old,
                },
                VoteEvent::Vote {
                    round: vote_round,
                    voter: vote_voter,
                    parameter: vote_parameter,
                    ..
                },
            ) = (pair[0], pair[1])
            else {
                return Err(inconsistent(
                    "Core Space governance replacement events are not Revoke/Vote pairs",
                ));
            };
            if usize::from(parameter) >= previous.len()
                || usize::from(parameter) >= parameter_count
                || !submitted[usize::from(parameter)]
                || events_seen[usize::from(parameter)]
                || round != operation.round
                || vote_round != operation.round
                || voter != expected_voter
                || vote_voter != expected_voter
                || parameter != vote_parameter
                || previous[usize::from(parameter)] != old
            {
                return Err(inconsistent(
                    "Core Space governance replacement evidence does not match state",
                ));
            }
            events_seen[usize::from(parameter)] = true;
        }
    } else {
        if operation.events.len() != parameter_count {
            return Err(inconsistent(
                "Core Space new-round governance events do not cover all active parameters",
            ));
        }
        let mut events_seen = [false; 4];
        for event in &operation.events {
            let VoteEvent::Vote {
                round,
                voter,
                parameter,
                allocation,
            } = *event
            else {
                return Err(inconsistent(
                    "Core Space new-round governance events contained Revoke",
                ));
            };
            let parameter_index = usize::from(parameter);
            let expected = operation
                .votes
                .iter()
                .find(|(index, _)| *index == parameter)
                .map(|(_, value)| *value)
                .unwrap_or_default();
            if parameter_index >= parameter_count
                || parameter_index >= events_seen.len()
                || events_seen[parameter_index]
                || operation.events.iter().filter(|event| matches!(event, VoteEvent::Vote { parameter: value, .. } if usize::from(*value) == parameter_index)).count() != 1
                || round != operation.round || voter != expected_voter || allocation != expected {
                return Err(inconsistent("Core Space new-round governance event does not match submitted vote"));
            }
            events_seen[parameter_index] = true;
        }
        if events_seen[..parameter_count].iter().any(|seen| !seen) {
            return Err(inconsistent(
                "Core Space new-round governance events omitted an active parameter",
            ));
        }
    }
    Ok(())
}

fn parameter(index: u16) -> Result<GovernanceParameter, CoreSpaceProtocolError> {
    match index {
        0 => Ok(GovernanceParameter::PowBaseReward),
        1 => Ok(GovernanceParameter::PosRewardInterestRate),
        2 => Ok(GovernanceParameter::StoragePointProportion),
        3 => Ok(GovernanceParameter::BaseFeeShareProportion),
        _ => Err(CoreSpaceProtocolError::unsupported_operation(format!(
            "Core Space governance call used unknown parameter {index}"
        ))),
    }
}

fn core_address(
    address: Address,
    execution: &CoreSpaceExecutedTransaction,
) -> Result<conflux_provider::CoreAddress, CoreSpaceProtocolError> {
    conflux_provider::CoreAddress::from_bytes(address.0, execution.address_network()).map_err(
        |error| {
            inconsistent(format!(
                "invalid Core Space governance voter address: {error}"
            ))
        },
    )
}

fn state_error(error: crate::core_space::CoreSpaceStateAccessError) -> CoreSpaceProtocolError {
    CoreSpaceProtocolError::state_access("read Core Space governance state", error)
}

fn inconsistent(details: impl Into<String>) -> CoreSpaceProtocolError {
    CoreSpaceProtocolError::inconsistent_execution(details)
}

use cfx_types::{Address, Space};
use cfx_vm_types::CallType;

use super::{VoteEvent, codec::decode_cast_vote, codec::decode_vote_event};
use crate::{
    core_space::{CoreSpaceChangesError, CoreSpaceExecutionPosition},
    execution::{CommittedExecutionTrace, FrameAction, TraceEvent},
};

#[derive(Debug, Clone)]
pub(super) struct GovernanceOperation {
    pub(super) position: CoreSpaceExecutionPosition,
    pub(super) voter: Address,
    pub(super) round: u64,
    pub(super) votes: Vec<(u16, super::VoteAllocation)>,
    pub(super) events: Vec<VoteEvent>,
}

pub(super) fn collect_operations(
    trace: &CommittedExecutionTrace,
    active: bool,
) -> Result<Vec<GovernanceOperation>, CoreSpaceChangesError> {
    if !active {
        return Ok(Vec::new());
    }
    let params = cfx_parameters::internal_contract_addresses::PARAMS_CONTROL_CONTRACT_ADDRESS;
    let mut operations = Vec::new();
    for event in trace.events() {
        let TraceEvent::FrameStart { position, frame_id } = event else {
            continue;
        };
        let frame = trace.frame(*frame_id);
        let FrameAction::Call {
            caller,
            target,
            code_address,
            transferred_value,
            call_type,
            calldata,
            ..
        } = &frame.action
        else {
            continue;
        };
        if *target != params && *code_address != params {
            continue;
        }
        let Some(call) = decode_cast_vote(calldata)? else {
            continue;
        };
        if frame.space != Space::Native
            || *call_type != CallType::Call
            || !transferred_value.is_zero()
        {
            return Err(CoreSpaceChangesError::unsupported_operation(
                "Core Space governance call did not use the canonical native plain-call form",
            ));
        }
        let mut logs = Vec::new();
        for event in trace.events() {
            let TraceEvent::Log {
                frame_id: event_frame,
                address,
                topics,
                data,
                ..
            } = event
            else {
                continue;
            };
            if *event_frame != *frame_id || *address != params {
                continue;
            }
            if let Some(event) = decode_vote_event(topics, data)? {
                logs.push(event);
            }
        }
        operations.push(GovernanceOperation {
            position: CoreSpaceExecutionPosition::from_index(*position),
            voter: *caller,
            round: call.round,
            votes: call.votes,
            events: logs,
        });
    }
    Ok(operations)
}

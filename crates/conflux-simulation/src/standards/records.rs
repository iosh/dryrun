use alloy_primitives::Bytes;
use cfx_types::Space;
use cfx_vm_types::CallType;
use contract_standards::legacy::{Position, Record, StandardCandidate, collect_candidates};

use crate::{
    ConfluxSimulationError,
    execution::{CommittedExecutionTrace, FrameAction, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx, u256_from_cfx},
};

pub(crate) fn collect_standard_candidates(
    trace: &CommittedExecutionTrace,
    analysis_space: Space,
) -> Result<Vec<StandardCandidate>, ConfluxSimulationError> {
    collect_candidates(&collect_standard_records(trace, analysis_space))
        .map_err(ConfluxSimulationError::from)
}

fn collect_standard_records(trace: &CommittedExecutionTrace, analysis_space: Space) -> Vec<Record> {
    trace
        .events()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::FrameStart {
                position: event_position,
                frame_id,
            } => {
                let frame = trace.frame(*frame_id);
                let FrameAction::Call {
                    call_type: CallType::Call,
                    caller,
                    target: call_target,
                    transferred_value,
                    calldata_len,
                    calldata_prefix,
                    ..
                } = &frame.action
                else {
                    return None;
                };
                (frame.space == analysis_space).then(|| Record::Call {
                    position: Position::new(*event_position, 0),
                    caller: address_from_cfx(*caller),
                    target: address_from_cfx(*call_target),
                    value: u256_from_cfx(*transferred_value),
                    input_len: *calldata_len,
                    input_prefix: Bytes::from(calldata_prefix.clone()),
                })
            }
            TraceEvent::Log {
                position: event_position,
                frame_id,
                address,
                topics,
                data: log_data,
            } => (trace.frame(*frame_id).space == analysis_space).then(|| Record::Log {
                position: Position::new(*event_position, 0),
                address: address_from_cfx(*address),
                topics: topics.iter().copied().map(b256_from_cfx).collect(),
                data: Bytes::from(log_data.clone()),
            }),
            TraceEvent::InternalTransfer { .. } => None,
        })
        .collect()
}

use alloy_primitives::Address;
use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::{MetadataValues, decode_standard_log, metadata_calls};

use crate::{
    ConfluxSimulationError,
    execution::{
        CommittedExecutionTrace, ConfluxExecutionOutput, PreparedTransactionExecution, TraceEvent,
    },
    primitive::{address_from_cfx, b256_from_cfx},
    standards::{StandardReadCallOutcome, execute_standard_read_call},
};

use super::{ChangePosition, PositionedCoreSpaceChange};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(crate) fn collect_standard_changes(
    output: &ConfluxExecutionOutput,
) -> Result<Vec<PositionedCoreSpaceChange>, ConfluxSimulationError> {
    verify_committed_logs(&output.trace, &output.logs)?;
    Ok(output
        .trace
        .events()
        .iter()
        .filter_map(|event| {
            let TraceEvent::Log {
                position,
                frame_id,
                address,
                topics,
                data,
            } = event
            else {
                return None;
            };
            if output.trace.frame(*frame_id).space != Space::Native {
                return None;
            }
            let address = address_from_cfx(*address);
            let topics = topics
                .iter()
                .copied()
                .map(b256_from_cfx)
                .collect::<Vec<_>>();
            decode_standard_log(address, &topics, data, |value| value).map(|change| {
                PositionedCoreSpaceChange::standard(ChangePosition::new(*position, 0), change)
            })
        })
        .collect())
}

pub(crate) fn load_standard_metadata(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    changes: &[PositionedCoreSpaceChange],
) -> Result<MetadataValues<Address>, ConfluxSimulationError> {
    let decoded = changes
        .iter()
        .filter_map(PositionedCoreSpaceChange::decoded_standard_log);
    let calls = metadata_calls(decoded);
    let mut values = MetadataValues::default();

    for (index, call) in calls.into_iter().enumerate() {
        if index >= MAX_METADATA_CALLS {
            values.record_unavailable(call);
            continue;
        }
        let outcome = execute_standard_read_call(
            state,
            machine,
            prepared_execution,
            *call.contract_address(),
            call.call_data(),
        )?;
        match outcome {
            StandardReadCallOutcome::Success(output)
                if output.len() <= MAX_METADATA_OUTPUT_BYTES =>
            {
                values.record_output(call, &output);
            }
            StandardReadCallOutcome::Success(_)
            | StandardReadCallOutcome::Revert
            | StandardReadCallOutcome::Halt => values.record_unavailable(call),
        }
    }

    Ok(values)
}

fn verify_committed_logs(
    trace: &CommittedExecutionTrace,
    committed_logs: &[primitives::LogEntry],
) -> Result<(), ConfluxSimulationError> {
    let trace_logs = trace.events().iter().filter_map(|event| {
        let TraceEvent::Log {
            frame_id,
            address,
            topics,
            data,
            ..
        } = event
        else {
            return None;
        };
        Some((trace.frame(*frame_id).space, address, topics, data))
    });
    let trace_log_count = trace_logs.clone().count();
    if trace_log_count != committed_logs.len() {
        return Err(ConfluxSimulationError::analysis_failed(format!(
            "Core Space trace contains {trace_log_count} committed logs, executor returned {}",
            committed_logs.len()
        )));
    }

    for (index, ((space, address, topics, data), committed)) in
        trace_logs.zip(committed_logs).enumerate()
    {
        if space != committed.space
            || *address != committed.address
            || topics != &committed.topics
            || data.as_slice() != committed.data.as_slice()
        {
            return Err(ConfluxSimulationError::analysis_failed(format!(
                "Core Space trace log {index} does not match the committed executor log"
            )));
        }
    }

    Ok(())
}

mod read_call;

use std::collections::HashSet;

use alloy_primitives::Address;
use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::{MetadataCall, MetadataValues, decode_standard_log, metadata_calls};

use crate::{
    core_space::CoreSpaceChangesError,
    espace::{MetadataReadError, ReadCallOutcome, execute_read_call},
    execution::{ConfluxExecutionOutput, PreparedTransactionExecution, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

use self::read_call::{StandardReadCallOutcome, execute_standard_read_call};
use super::{ChangePosition, PositionedCoreSpaceChange};

const MAX_METADATA_CALLS: usize = 64;
const MAX_METADATA_OUTPUT_BYTES: usize = 4 * 1024;

pub(crate) fn collect_standard_changes(
    output: &ConfluxExecutionOutput,
) -> Vec<PositionedCoreSpaceChange> {
    output
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
        .collect()
}

pub(crate) fn load_standard_metadata(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    changes: &[PositionedCoreSpaceChange],
    nested_espace_calls: Vec<(usize, MetadataCall<Address>)>,
    espace_sender: Address,
) -> Result<SpaceMetadataValues, CoreSpaceChangesError> {
    let mut calls = Vec::new();
    for change in changes {
        let Some(decoded) = change.decoded_standard_log() else {
            continue;
        };
        calls.extend(
            metadata_calls(std::iter::once(decoded))
                .into_iter()
                .map(|call| PositionedMetadataCall {
                    position: change.position(),
                    space: MetadataSpace::Core,
                    call,
                }),
        );
    }
    calls.extend(
        nested_espace_calls
            .into_iter()
            .map(|(position, call)| PositionedMetadataCall {
                position: ChangePosition::new(position, 0),
                space: MetadataSpace::Espace,
                call,
            }),
    );
    calls.sort_by_key(|call| call.position);

    let mut values = SpaceMetadataValues::default();
    let mut seen_core = HashSet::new();
    let mut seen_espace = HashSet::new();
    let mut call_count = 0;

    for positioned_call in calls {
        let PositionedMetadataCall { space, call, .. } = positioned_call;
        let is_new = match space {
            MetadataSpace::Core => seen_core.insert(call.clone()),
            MetadataSpace::Espace => seen_espace.insert(call.clone()),
        };
        if !is_new {
            continue;
        }

        let target_values = match space {
            MetadataSpace::Core => &mut values.core,
            MetadataSpace::Espace => &mut values.espace,
        };
        if call_count >= MAX_METADATA_CALLS {
            target_values.record_unavailable(call);
            continue;
        }
        call_count += 1;

        match space {
            MetadataSpace::Core => {
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
                        target_values.record_output(call, &output);
                    }
                    StandardReadCallOutcome::Success(_)
                    | StandardReadCallOutcome::Revert
                    | StandardReadCallOutcome::Halt => {
                        target_values.record_unavailable(call);
                    }
                }
            }
            MetadataSpace::Espace => {
                let outcome = execute_read_call(
                    state,
                    machine,
                    prepared_execution,
                    espace_sender,
                    *call.contract_address(),
                    call.call_data(),
                    &call,
                )
                .map_err(map_espace_metadata_error)?;
                match outcome {
                    ReadCallOutcome::Success(output)
                        if output.len() <= MAX_METADATA_OUTPUT_BYTES =>
                    {
                        target_values.record_output(call, &output);
                    }
                    ReadCallOutcome::Success(_)
                    | ReadCallOutcome::Reverted(_)
                    | ReadCallOutcome::Failed => target_values.record_unavailable(call),
                }
            }
        }
    }

    Ok(values)
}

#[derive(Default)]
pub(crate) struct SpaceMetadataValues {
    pub(crate) core: MetadataValues<Address>,
    pub(crate) espace: MetadataValues<Address>,
}

#[derive(Clone, Copy)]
enum MetadataSpace {
    Core,
    Espace,
}

struct PositionedMetadataCall {
    position: ChangePosition,
    space: MetadataSpace,
    call: MetadataCall<Address>,
}

fn map_espace_metadata_error(error: MetadataReadError) -> CoreSpaceChangesError {
    match error {
        MetadataReadError::StateAccess { call, source } => CoreSpaceChangesError::state_read(
            format!("execute nested eSpace metadata probe {call:?}"),
            source,
        ),
        MetadataReadError::ProbeExecution { call, details } => {
            CoreSpaceChangesError::internal_invariant(format!(
                "nested eSpace metadata probe {call:?} could not be isolated: {details}"
            ))
        }
    }
}

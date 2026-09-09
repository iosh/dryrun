mod metadata;
mod read_call;
mod token_changes;

use cfx_types::Space;
use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::{
    espace::EspaceChangesError,
    execution::{CommittedExecutionTrace, FrameId, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

use metadata::load_metadata;
pub(crate) use read_call::{
    IsolatedReadCallError, MetadataReadError, ReadCallOutcome, execute_isolated_read_call,
    execute_read_call,
};
pub(crate) use token_changes::{VerifiedChange, WrappedOperation, derive_verified_changes};

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) position: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy_primitives::Address>,
}

pub(super) fn decode_standard_occurrences_in_scope(
    trace: &CommittedExecutionTrace,
    includes_frame: impl Fn(FrameId) -> bool,
) -> Result<Vec<DecodedStandardOccurrence>, EspaceChangesError> {
    let mut occurrences = Vec::new();
    for event in trace.events() {
        let TraceEvent::Log {
            position,
            frame_id,
            address,
            topics,
            data,
        } = event
        else {
            continue;
        };
        if trace.frame(*frame_id).space != Space::Ethereum {
            continue;
        }
        if !includes_frame(*frame_id) {
            continue;
        }
        let address = address_from_cfx(*address);
        let topics = topics
            .iter()
            .copied()
            .map(b256_from_cfx)
            .collect::<Vec<_>>();
        let Some(decoded_log) = decode_standard_log(address, &topics, data, |address| address)
            .map_err(|error| EspaceChangesError::derivation("token", error))?
        else {
            continue;
        };
        occurrences.push(DecodedStandardOccurrence {
            position: *position,
            decoded_log,
        });
    }
    Ok(occurrences)
}

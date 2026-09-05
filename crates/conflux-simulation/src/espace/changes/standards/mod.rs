mod metadata;
mod read_call;

use cfx_types::Space;
use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::{
    espace::{EspaceCommittedLog, EspaceExecutionSpace},
    execution::{CommittedExecutionTrace, FrameId, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

pub(super) use metadata::load_metadata;
pub(crate) use read_call::{
    IsolatedReadCallError, MetadataReadError, ReadCallOutcome, execute_isolated_read_call,
    execute_read_call,
};

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) position: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy_primitives::Address>,
}

pub(super) fn decode_standard_occurrences(
    logs: &[EspaceCommittedLog],
) -> Vec<DecodedStandardOccurrence> {
    logs.iter()
        .filter(|log| log.space() == EspaceExecutionSpace::Espace)
        .filter_map(|log| {
            decode_standard_log(log.address(), log.topics(), log.data(), |address| address).map(
                |decoded_log| DecodedStandardOccurrence {
                    position: log.position().index(),
                    decoded_log,
                },
            )
        })
        .collect()
}

pub(super) fn decode_standard_occurrences_in_scope(
    trace: &CommittedExecutionTrace,
    includes_frame: impl Fn(FrameId) -> bool,
) -> Vec<DecodedStandardOccurrence> {
    trace
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
            if trace.frame(*frame_id).space != Space::Ethereum {
                return None;
            }
            if !includes_frame(*frame_id) {
                return None;
            }
            let address = address_from_cfx(*address);
            let topics = topics
                .iter()
                .copied()
                .map(b256_from_cfx)
                .collect::<Vec<_>>();
            decode_standard_log(address, &topics, data, |address| address).map(|decoded_log| {
                DecodedStandardOccurrence {
                    position: *position,
                    decoded_log,
                }
            })
        })
        .collect()
}

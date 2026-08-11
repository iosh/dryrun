mod metadata;
mod read_call;

use cfx_types::Space;
use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::{
    execution::{CommittedExecutionTrace, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

pub(super) use metadata::load_metadata;

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) position: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy_primitives::Address>,
}

pub(super) fn decode_standard_occurrences(
    trace: &CommittedExecutionTrace,
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

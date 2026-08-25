mod metadata;
mod read_call;

use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::EvmExecutionEvent;

pub(super) use metadata::load_metadata;

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) event_index: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy::primitives::Address>,
}

pub(super) fn decode_standard_occurrences(
    events: &[EvmExecutionEvent],
) -> Vec<DecodedStandardOccurrence> {
    events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let EvmExecutionEvent::Log {
                address,
                topics,
                data,
            } = event
            else {
                return None;
            };

            decode_standard_log(*address, topics, data, |address| address).map(|decoded_log| {
                DecodedStandardOccurrence {
                    event_index,
                    decoded_log,
                }
            })
        })
        .collect()
}

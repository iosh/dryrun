mod metadata;
mod read_call;

use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::EvmExecutionObservation;

pub(super) use metadata::load_metadata;

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) observation_index: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy::primitives::Address>,
}

pub(super) fn decode_standard_occurrences(
    observations: &[EvmExecutionObservation],
) -> Vec<DecodedStandardOccurrence> {
    observations
        .iter()
        .enumerate()
        .filter_map(|(observation_index, observation)| {
            let EvmExecutionObservation::Log {
                address,
                topics,
                data,
            } = observation
            else {
                return None;
            };

            decode_standard_log(*address, topics, data, |address| address).map(|decoded_log| {
                DecodedStandardOccurrence {
                    observation_index,
                    decoded_log,
                }
            })
        })
        .collect()
}

mod metadata;
mod read_call;

use cfx_types::Space;
use contract_standards::{DecodedStandardLog, decode_standard_log};

use crate::{
    execution::Observation,
    primitive::{address_from_cfx, b256_from_cfx},
};

pub(super) use metadata::load_metadata;

#[derive(Debug)]
pub(super) struct DecodedStandardOccurrence {
    pub(super) position: usize,
    pub(super) decoded_log: DecodedStandardLog<alloy_primitives::Address>,
}

pub(super) fn decode_standard_occurrences(
    observations: &[Observation],
) -> Vec<DecodedStandardOccurrence> {
    observations
        .iter()
        .filter_map(|observation| {
            let Observation::Log {
                position,
                space: Space::Ethereum,
                address,
                topics,
                data,
            } = observation
            else {
                return None;
            };
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

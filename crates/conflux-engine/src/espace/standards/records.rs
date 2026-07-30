use alloy_primitives::Bytes;
use cfx_types::Space;
use cfx_vm_types::CallType;
use contract_standards::{Position, Record, StandardCandidate, collect_candidates};

use crate::{
    ConfluxEngineError,
    execution::Observation,
    primitive::{address_from_cfx, b256_from_cfx, u256_from_cfx},
};

pub(crate) fn collect_standard_candidates(
    observations: Vec<Observation>,
) -> Result<Vec<StandardCandidate>, ConfluxEngineError> {
    collect_candidates(&collect_records(observations)).map_err(ConfluxEngineError::from)
}

fn collect_records(observations: Vec<Observation>) -> Vec<Record> {
    observations
        .into_iter()
        .filter_map(|observation| match observation {
            Observation::Call {
                position,
                space: Space::Ethereum,
                call_type: CallType::Call,
                caller,
                target,
                transferred_value,
                input_len,
                input_prefix,
                ..
            } => Some(Record::Call {
                position: Position::new(position, 0),
                caller: address_from_cfx(caller),
                target: address_from_cfx(target),
                value: u256_from_cfx(transferred_value),
                input_len,
                input_prefix: Bytes::from(input_prefix),
            }),
            Observation::Log {
                position,
                space: Space::Ethereum,
                address,
                topics,
                data,
            } => Some(Record::Log {
                position: Position::new(position, 0),
                address: address_from_cfx(address),
                topics: topics.into_iter().map(b256_from_cfx).collect(),
                data: Bytes::from(data),
            }),
            Observation::Call { .. }
            | Observation::Log { .. }
            | Observation::InternalTransfer { .. } => None,
        })
        .collect()
}

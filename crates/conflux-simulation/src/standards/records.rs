use alloy_primitives::Bytes;
use cfx_types::Space;
use cfx_vm_types::CallType;
use contract_standards::legacy::{Position, Record, StandardCandidate, collect_candidates};

use crate::{
    ConfluxSimulationError,
    execution::Observation,
    primitive::{address_from_cfx, b256_from_cfx, u256_from_cfx},
};

pub(crate) fn collect_standard_candidates(
    execution_observations: &[Observation],
    analysis_space: Space,
) -> Result<Vec<StandardCandidate>, ConfluxSimulationError> {
    collect_candidates(&collect_standard_records(
        execution_observations,
        analysis_space,
    ))
    .map_err(ConfluxSimulationError::from)
}

fn collect_standard_records(
    execution_observations: &[Observation],
    analysis_space: Space,
) -> Vec<Record> {
    execution_observations
        .iter()
        .filter_map(|observation| match observation {
            Observation::Call {
                position: observation_position,
                space: observation_space,
                call_type: CallType::Call,
                caller,
                target: call_target,
                transferred_value,
                input_len,
                input_prefix,
                ..
            } if *observation_space == analysis_space => Some(Record::Call {
                position: Position::new(*observation_position, 0),
                caller: address_from_cfx(*caller),
                target: address_from_cfx(*call_target),
                value: u256_from_cfx(*transferred_value),
                input_len: *input_len,
                input_prefix: Bytes::from(input_prefix.clone()),
            }),
            Observation::Log {
                position: observation_position,
                space: observation_space,
                address,
                topics,
                data: log_data,
            } if *observation_space == analysis_space => Some(Record::Log {
                position: Position::new(*observation_position, 0),
                address: address_from_cfx(*address),
                topics: topics.iter().copied().map(b256_from_cfx).collect(),
                data: Bytes::from(log_data.clone()),
            }),
            Observation::Call { .. }
            | Observation::CreateTransfer { .. }
            | Observation::Log { .. }
            | Observation::InternalTransfer { .. } => None,
        })
        .collect()
}

use contract_standards::legacy::{Position, Record, StandardCandidate, collect_candidates};

use crate::{EvmExecutionObservation, EvmSimulationError};

pub(crate) fn collect_standard_candidates(
    observations: &[EvmExecutionObservation],
) -> Result<Vec<StandardCandidate>, EvmSimulationError> {
    Ok(collect_candidates(&collect_records(observations))?)
}

fn collect_records(observations: &[EvmExecutionObservation]) -> Vec<Record> {
    observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| {
            let position = Position::new(index, 0);

            match observation {
                EvmExecutionObservation::Call {
                    caller,
                    target,
                    value,
                    input_len,
                    input_prefix,
                    ..
                } => Some(Record::Call {
                    position,
                    caller: *caller,
                    target: *target,
                    value: *value,
                    input_len: *input_len,
                    input_prefix: input_prefix.clone(),
                }),
                EvmExecutionObservation::Log {
                    address,
                    topics,
                    data,
                } => Some(Record::Log {
                    position,
                    address: *address,
                    topics: topics.clone(),
                    data: data.clone(),
                }),
                EvmExecutionObservation::CreateTransfer { .. }
                | EvmExecutionObservation::SelfDestruct { .. } => None,
            }
        })
        .collect()
}

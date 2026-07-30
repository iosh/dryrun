use contract_standards::{Position, Record, StandardCandidate, collect_candidates};

use crate::EvmEngineError;

use super::observation::Observation;

pub(crate) fn collect_contract_candidates(
    observations: &[Observation],
) -> Result<Vec<StandardCandidate>, EvmEngineError> {
    Ok(collect_candidates(&collect_records(observations))?)
}

fn collect_records(observations: &[Observation]) -> Vec<Record> {
    observations
        .iter()
        .enumerate()
        .filter_map(|(index, observation)| {
            let position = Position::new(index, 0);

            match observation {
                Observation::Call {
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
                Observation::Log {
                    address,
                    topics,
                    data,
                } => Some(Record::Log {
                    position,
                    address: *address,
                    topics: topics.clone(),
                    data: data.clone(),
                }),
                Observation::CreateTransfer { .. } | Observation::SelfDestruct { .. } => None,
            }
        })
        .collect()
}

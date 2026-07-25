use std::{collections::HashMap, sync::LazyLock};

use alloy_primitives::{Address, B256, U256, keccak256};
use contract_standards::{
    Position, Record, StandardCandidate, collect_candidates, sort_candidates_by_position,
};

use crate::EvmEngineError;

use super::observation::Observation;

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ValueCall {
    from: Address,
    to: Address,
    amount: U256,
}

enum WrappedNativeMovement {
    Deposit {
        token: Address,
        account: Address,
        amount: U256,
    },
    Withdrawal {
        token: Address,
        account: Address,
        amount: U256,
    },
}

pub(crate) fn collect_contract_candidates(
    observations: &[Observation],
) -> Result<Vec<StandardCandidate>, EvmEngineError> {
    let records = collect_records(observations);
    let mut candidates = collect_candidates(&records)?;

    append_wrapped_native_candidates(observations, &mut candidates);
    sort_candidates_by_position(&mut candidates);

    Ok(candidates)
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
                    input_len,
                    input_prefix,
                    ..
                } => Some(Record::Call {
                    position,
                    caller: *caller,
                    target: *target,
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

fn append_wrapped_native_candidates(
    observations: &[Observation],
    candidates: &mut Vec<StandardCandidate>,
) {
    let mut value_calls = HashMap::new();

    for (index, observation) in observations.iter().enumerate() {
        record_value_call(observation, &mut value_calls);

        let Some(movement) = decode_wrapped_native_movement(observation) else {
            continue;
        };

        let (token, from, to, amount) = match movement {
            WrappedNativeMovement::Deposit {
                token,
                account,
                amount,
            } if consume_value_call(
                &mut value_calls,
                ValueCall {
                    from: account,
                    to: token,
                    amount,
                },
            ) =>
            {
                (token, Address::ZERO, account, amount)
            }
            WrappedNativeMovement::Withdrawal {
                token,
                account,
                amount,
            } if consume_value_call(
                &mut value_calls,
                ValueCall {
                    from: token,
                    to: account,
                    amount,
                },
            ) =>
            {
                (token, account, Address::ZERO, amount)
            }
            WrappedNativeMovement::Deposit { .. } | WrappedNativeMovement::Withdrawal { .. } => {
                continue;
            }
        };

        candidates.push(StandardCandidate::erc20_movement(
            Position::new(index, 0),
            token,
            from,
            to,
            amount,
        ));
    }
}

fn record_value_call(observation: &Observation, value_calls: &mut HashMap<ValueCall, usize>) {
    let Observation::Call {
        caller,
        target,
        value,
        ..
    } = observation
    else {
        return;
    };

    if value.is_zero() {
        return;
    }

    *value_calls
        .entry(ValueCall {
            from: *caller,
            to: *target,
            amount: *value,
        })
        .or_default() += 1;
}

fn consume_value_call(value_calls: &mut HashMap<ValueCall, usize>, call: ValueCall) -> bool {
    let Some(count) = value_calls.get_mut(&call) else {
        return false;
    };

    if *count == 0 {
        return false;
    }

    *count -= 1;
    true
}

fn decode_wrapped_native_movement(observation: &Observation) -> Option<WrappedNativeMovement> {
    let Observation::Log {
        address,
        topics,
        data,
    } = observation
    else {
        return None;
    };

    let topic0 = topics.first()?;
    let (account, amount) = decode_wrapped_native_event(topics, data)?;

    if *topic0 == *DEPOSIT_TOPIC0 {
        Some(WrappedNativeMovement::Deposit {
            token: *address,
            account,
            amount,
        })
    } else if *topic0 == *WITHDRAWAL_TOPIC0 {
        Some(WrappedNativeMovement::Withdrawal {
            token: *address,
            account,
            amount,
        })
    } else {
        None
    }
}

fn decode_wrapped_native_event(topics: &[B256], data: &[u8]) -> Option<(Address, U256)> {
    if topics.len() != 2 || data.len() != 32 {
        return None;
    }

    let topic = &topics[1];
    if topic.as_slice()[..12].iter().any(|byte| *byte != 0) {
        return None;
    }

    Some((Address::from_word(*topic), U256::from_be_slice(data)))
}

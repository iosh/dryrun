use std::{collections::HashSet, sync::LazyLock};

use crate::change_observation::Observation;
use alloy_primitives::{Address, U256, keccak256};

use super::{
    candidate::{
        ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence, ObservationPosition,
    },
    error::TransactionChangesError,
    event_codec::{DecodedEvent, decode_event},
};

const TRANSFER_FROM_INPUT_LEN: usize = 100;

static TRANSFER_FROM_SELECTOR: LazyLock<[u8; 4]> = LazyLock::new(|| {
    let hash = keccak256("transferFrom(address,address,uint256)");
    [hash[0], hash[1], hash[2], hash[3]]
});

pub(crate) fn collect_candidates(
    observations: &[Observation],
) -> Result<Vec<ChangeCandidate>, TransactionChangesError> {
    let (decoded_events, erc20_transfer_tokens) = decode_observations(observations)?;

    let mut candidates = Vec::new();

    for (observation_index, (observation, decoded_event)) in
        observations.iter().zip(decoded_events).enumerate()
    {
        append_native_candidate(observation_index, observation, &mut candidates)?;

        append_transfer_from_candidate(
            observation_index,
            observation,
            &erc20_transfer_tokens,
            &mut candidates,
        );
        if let Some(event) = decoded_event {
            append_event_candidates(observation_index, event, &mut candidates);
        }
    }

    Ok(candidates)
}

fn decode_observations(
    observations: &[Observation],
) -> Result<(Vec<Option<DecodedEvent>>, HashSet<Address>), TransactionChangesError> {
    let mut decoded_events = Vec::with_capacity(observations.len());
    let mut erc20_transfer_tokens = HashSet::new();

    for (observation_index, observation) in observations.iter().enumerate() {
        let decoded_event = decode_event(observation).map_err(|source| {
            TransactionChangesError::MalformedEvent {
                observation_index,
                source,
            }
        })?;

        if let Some(DecodedEvent::Erc20Transfer { token, .. }) = &decoded_event {
            erc20_transfer_tokens.insert(*token);
        }

        decoded_events.push(decoded_event);
    }

    Ok((decoded_events, erc20_transfer_tokens))
}

fn append_transfer_from_candidate(
    observation_index: usize,
    observation: &Observation,
    erc20_transfer_tokens: &HashSet<Address>,
    candidates: &mut Vec<ChangeCandidate>,
) {
    let Observation::Call {
        caller,
        target,
        input_len,
        input_prefix,
        ..
    } = observation
    else {
        return;
    };

    if !erc20_transfer_tokens.contains(target) {
        return;
    }

    let Some((owner, amount)) = decode_transfer_from_call(*input_len, input_prefix.as_ref()) else {
        return;
    };

    candidates.push(ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index: 0,
        },
        kind: ChangeCandidateKind::Erc20Allowance {
            token: *target,
            owner,
            spender: *caller,
            evidence: Erc20AllowanceEvidence::TransferFromCall { amount },
        },
    });
}

fn append_native_candidate(
    observation_index: usize,
    observation: &Observation,
    candidates: &mut Vec<ChangeCandidate>,
) -> Result<(), TransactionChangesError> {
    let kind = match observation {
        Observation::Call {
            caller,
            target,
            value,
            ..
        } if !value.is_zero() => Some(ChangeCandidateKind::NativeTransfer {
            from: *caller,
            to: *target,
            amount: *value,
        }),

        Observation::CreateTransfer { from, to, amount } if !amount.is_zero() => {
            Some(ChangeCandidateKind::NativeTransfer {
                from: *from,
                to: *to,
                amount: *amount,
            })
        }

        Observation::SelfDestruct { amount, .. } if amount.is_zero() => None,

        Observation::SelfDestruct {
            contract,
            target,
            amount,
        } if contract == target => {
            return Err(TransactionChangesError::UnsupportedSelfDestructToSelf {
                observation_index,
                contract: *contract,
                amount: *amount,
            });
        }

        Observation::SelfDestruct {
            contract,
            target,
            amount,
        } => Some(ChangeCandidateKind::NativeTransfer {
            from: *contract,
            to: *target,
            amount: *amount,
        }),

        Observation::Call { .. } | Observation::CreateTransfer { .. } | Observation::Log { .. } => {
            None
        }
    };

    if let Some(kind) = kind {
        candidates.push(ChangeCandidate {
            position: ObservationPosition {
                observation_index,
                item_index: 0,
            },
            kind,
        });
    }

    Ok(())
}

fn append_event_candidates(
    observation_index: usize,
    event: DecodedEvent,
    candidates: &mut Vec<ChangeCandidate>,
) {
    let mut push = |item_index, kind| {
        candidates.push(ChangeCandidate {
            position: ObservationPosition {
                observation_index,
                item_index,
            },
            kind,
        });
    };

    match event {
        DecodedEvent::Erc20Transfer {
            token,
            from,
            to,
            amount,
        } => push(
            0,
            ChangeCandidateKind::Erc20Transfer {
                token,
                from,
                to,
                amount,
            },
        ),

        DecodedEvent::Erc721Transfer {
            collection,
            from,
            to,
            token_id,
        } => push(
            0,
            ChangeCandidateKind::Erc721Transfer {
                collection,
                from,
                to,
                token_id,
            },
        ),

        DecodedEvent::Erc1155TransferSingle {
            collection,
            from,
            to,
            token_id,
            amount,
        } => push(
            0,
            ChangeCandidateKind::Erc1155Transfer {
                collection,
                from,
                to,
                token_id,
                amount,
            },
        ),

        DecodedEvent::Erc1155TransferBatch {
            collection,
            from,
            to,
            items,
        } => {
            for (item_index, item) in items.into_iter().enumerate() {
                push(
                    item_index,
                    ChangeCandidateKind::Erc1155Transfer {
                        collection,
                        from,
                        to,
                        token_id: item.token_id,
                        amount: item.amount,
                    },
                );
            }
        }

        DecodedEvent::Erc20Approval {
            token,
            owner,
            spender,
            value,
        } => push(
            0,
            ChangeCandidateKind::Erc20Allowance {
                token,
                owner,
                spender,
                evidence: Erc20AllowanceEvidence::ApprovalEvent { value },
            },
        ),

        DecodedEvent::Erc721Approval {
            collection,
            owner,
            approved_address,
            token_id,
        } => push(
            0,
            ChangeCandidateKind::Erc721Approval {
                collection,
                owner,
                approved_address: (approved_address != Address::ZERO).then_some(approved_address),
                token_id,
            },
        ),

        DecodedEvent::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        } => push(
            0,
            ChangeCandidateKind::OperatorApproval {
                collection,
                owner,
                operator,
                approved,
            },
        ),
    }
}

fn decode_transfer_from_call(input_len: usize, input_prefix: &[u8]) -> Option<(Address, U256)> {
    if input_len != TRANSFER_FROM_INPUT_LEN
        || input_prefix.len() != TRANSFER_FROM_INPUT_LEN
        || !input_prefix.starts_with(&*TRANSFER_FROM_SELECTOR)
    {
        return None;
    }

    let owner = calldata_address(&input_prefix[4..36])?;

    // Recipient is not part of the allowance relation, but it must still be
    // a valid ABI-encoded address before this is treated as standard calldata.
    calldata_address(&input_prefix[36..68])?;

    let amount = U256::from_be_slice(&input_prefix[68..100]);

    Some((owner, amount))
}

fn calldata_address(word: &[u8]) -> Option<Address> {
    if word.len() != 32 {
        return None;
    }

    if word[..12].iter().any(|byte| *byte != 0) {
        return None;
    }

    Some(Address::from_slice(&word[12..]))
}

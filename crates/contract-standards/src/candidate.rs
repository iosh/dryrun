use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use alloy_primitives::{Address, B256, Bytes, U256, keccak256};

use crate::{
    ContractStandardsError,
    event_codec::{DecodedEvent, decode_event},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub index: usize,
    pub item_index: usize,
}

impl Position {
    pub const fn new(index: usize, item_index: usize) -> Self {
        Self { index, item_index }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record {
    Call {
        position: Position,
        caller: Address,
        target: Address,
        value: U256,
        input_len: usize,
        input_prefix: Bytes,
    },
    Log {
        position: Position,
        address: Address,
        topics: Vec<B256>,
        data: Bytes,
    },
}

impl Record {
    pub const fn position(&self) -> Position {
        match self {
            Self::Call { position, .. } | Self::Log { position, .. } => *position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardCandidate {
    pub(crate) position: Position,
    pub(crate) kind: StandardCandidateKind,
}

impl StandardCandidate {
    pub fn erc20_movement(
        position: Position,
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
    ) -> Self {
        Self {
            position,
            kind: StandardCandidateKind::Erc20Movement {
                token,
                from,
                to,
                amount,
            },
        }
    }

    pub const fn position(&self) -> Position {
        self.position
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandardCandidateKind {
    Erc20Movement {
        token: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc721Transfer {
        collection: Address,
        from: Address,
        to: Address,
        token_id: U256,
    },
    Erc1155Transfer {
        collection: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc20Allowance {
        token: Address,
        owner: Address,
        spender: Address,
        source: AllowanceSource,
    },
    Erc721Approval {
        collection: Address,
        owner: Address,
        approved_address: Option<Address>,
        token_id: U256,
    },
    OperatorApproval {
        collection: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllowanceSource {
    ApprovalEvent { value: U256 },
    TransferFromCall { amount: U256 },
}

const TRANSFER_FROM_INPUT_LEN: usize = 100;

static TRANSFER_FROM_SELECTOR: LazyLock<[u8; 4]> = LazyLock::new(|| {
    let hash = keccak256("transferFrom(address,address,uint256)");
    [hash[0], hash[1], hash[2], hash[3]]
});
static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NativeValueCall {
    from: Address,
    to: Address,
    amount: U256,
}

enum WrappedNativeEvent {
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

pub fn collect_candidates(
    records: &[Record],
) -> Result<Vec<StandardCandidate>, ContractStandardsError> {
    let (decoded_events, erc20_transfer_tokens) = decode_records(records)?;
    let mut candidates = Vec::new();

    for (item, decoded_event) in records.iter().zip(decoded_events) {
        append_transfer_from_candidate(item, &erc20_transfer_tokens, &mut candidates);

        if let Some(event) = decoded_event {
            append_event_candidates(item.position(), event, &mut candidates);
        }
    }

    candidates.extend(wrapped_native_candidates(records));
    sort_candidates_by_position(&mut candidates);

    Ok(candidates)
}

pub fn sort_candidates_by_position(candidates: &mut [StandardCandidate]) {
    candidates.sort_by_key(StandardCandidate::position);
}

fn wrapped_native_candidates(records: &[Record]) -> Vec<StandardCandidate> {
    let mut value_calls = HashMap::new();

    for record in records {
        let Some(call) = native_value_call(record) else {
            continue;
        };
        *value_calls.entry(call).or_default() += 1;
    }

    records
        .iter()
        .filter_map(|record| {
            let event = decode_wrapped_native_log(record)?;

            let (token, from, to, amount, call) = match event {
                WrappedNativeEvent::Deposit {
                    token,
                    account,
                    amount,
                } => (
                    token,
                    Address::ZERO,
                    account,
                    amount,
                    NativeValueCall {
                        from: account,
                        to: token,
                        amount,
                    },
                ),
                WrappedNativeEvent::Withdrawal {
                    token,
                    account,
                    amount,
                } => (
                    token,
                    account,
                    Address::ZERO,
                    amount,
                    NativeValueCall {
                        from: token,
                        to: account,
                        amount,
                    },
                ),
            };

            take_value_call(&mut value_calls, call).then(|| {
                StandardCandidate::erc20_movement(record.position(), token, from, to, amount)
            })
        })
        .collect()
}

fn native_value_call(record: &Record) -> Option<NativeValueCall> {
    let Record::Call {
        caller,
        target,
        value,
        ..
    } = record
    else {
        return None;
    };

    if value.is_zero() {
        return None;
    }

    Some(NativeValueCall {
        from: *caller,
        to: *target,
        amount: *value,
    })
}

fn take_value_call(
    value_calls: &mut HashMap<NativeValueCall, usize>,
    call: NativeValueCall,
) -> bool {
    let Some(count) = value_calls.get_mut(&call) else {
        return false;
    };

    if *count == 0 {
        return false;
    }

    *count -= 1;
    true
}

fn decode_wrapped_native_log(record: &Record) -> Option<WrappedNativeEvent> {
    let Record::Log {
        address,
        topics,
        data,
        ..
    } = record
    else {
        return None;
    };

    let topic0 = topics.first()?;
    let (account, amount) = decode_wrapped_native_fields(topics, data)?;

    if *topic0 == *DEPOSIT_TOPIC0 {
        Some(WrappedNativeEvent::Deposit {
            token: *address,
            account,
            amount,
        })
    } else if *topic0 == *WITHDRAWAL_TOPIC0 {
        Some(WrappedNativeEvent::Withdrawal {
            token: *address,
            account,
            amount,
        })
    } else {
        None
    }
}

fn decode_wrapped_native_fields(topics: &[B256], data: &[u8]) -> Option<(Address, U256)> {
    if topics.len() != 2 || data.len() != 32 {
        return None;
    }

    let topic = &topics[1];
    if topic.as_slice()[..12].iter().any(|byte| *byte != 0) {
        return None;
    }

    Some((Address::from_word(*topic), U256::from_be_slice(data)))
}

fn decode_records(
    records: &[Record],
) -> Result<(Vec<Option<DecodedEvent>>, HashSet<Address>), ContractStandardsError> {
    let mut decoded_events = Vec::with_capacity(records.len());
    let mut erc20_transfer_tokens = HashSet::new();

    for item in records {
        let decoded_event =
            decode_event(item).map_err(|source| ContractStandardsError::MalformedEvent {
                position: item.position(),
                source,
            })?;

        if let Some(DecodedEvent::Erc20Transfer { token, .. }) = &decoded_event {
            erc20_transfer_tokens.insert(*token);
        }

        decoded_events.push(decoded_event);
    }

    Ok((decoded_events, erc20_transfer_tokens))
}

fn append_transfer_from_candidate(
    item: &Record,
    erc20_transfer_tokens: &HashSet<Address>,
    candidates: &mut Vec<StandardCandidate>,
) {
    let Record::Call {
        position,
        caller,
        target,
        input_len,
        input_prefix,
        ..
    } = item
    else {
        return;
    };

    if !erc20_transfer_tokens.contains(target) {
        return;
    }

    let Some((owner, amount)) = decode_transfer_from_call(*input_len, input_prefix.as_ref()) else {
        return;
    };

    candidates.push(StandardCandidate {
        position: *position,
        kind: StandardCandidateKind::Erc20Allowance {
            token: *target,
            owner,
            spender: *caller,
            source: AllowanceSource::TransferFromCall { amount },
        },
    });
}

fn append_event_candidates(
    position: Position,
    event: DecodedEvent,
    candidates: &mut Vec<StandardCandidate>,
) {
    let mut push = |item_index, kind| {
        candidates.push(StandardCandidate {
            position: Position::new(position.index, item_index),
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
            StandardCandidateKind::Erc20Movement {
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
            StandardCandidateKind::Erc721Transfer {
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
            StandardCandidateKind::Erc1155Transfer {
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
                    StandardCandidateKind::Erc1155Transfer {
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
            StandardCandidateKind::Erc20Allowance {
                token,
                owner,
                spender,
                source: AllowanceSource::ApprovalEvent { value },
            },
        ),
        DecodedEvent::Erc721Approval {
            collection,
            owner,
            approved_address,
            token_id,
        } => push(
            0,
            StandardCandidateKind::Erc721Approval {
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
            StandardCandidateKind::OperatorApproval {
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

    // Recipient is not part of the allowance relation, but its ABI encoding
    // must be valid before this becomes a standard call record.
    calldata_address(&input_prefix[36..68])?;

    let amount = U256::from_be_slice(&input_prefix[68..100]);

    Some((owner, amount))
}

fn calldata_address(word: &[u8]) -> Option<Address> {
    if word.len() != 32 || word[..12].iter().any(|byte| *byte != 0) {
        return None;
    }

    Some(Address::from_slice(&word[12..]))
}

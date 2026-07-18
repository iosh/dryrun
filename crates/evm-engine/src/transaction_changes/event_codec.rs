use std::{fmt, sync::LazyLock};

use alloy::sol_types::SolValue;
use alloy_primitives::{Address, B256, U256, keccak256};
use thiserror::Error;

use crate::change_observation::Observation;

static TRANSFER_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Transfer(address,address,uint256)"));
static APPROVAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Approval(address,address,uint256)"));
static APPROVAL_FOR_ALL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("ApprovalForAll(address,address,bool)"));
static TRANSFER_SINGLE_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("TransferSingle(address,address,address,uint256,uint256)"));
static TRANSFER_BATCH_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("TransferBatch(address,address,address,uint256[],uint256[])"));

pub(super) fn decode_event(
    observation: &Observation,
) -> Result<Option<DecodedEvent>, EventCodecError> {
    let Observation::Log {
        address,
        topics,
        data,
    } = observation
    else {
        return Ok(None);
    };

    let Some(topic0) = topics.first() else {
        return Ok(None);
    };

    let decoded = if *topic0 == *TRANSFER_TOPIC0 {
        decode_transfer_event(*address, topics, data)?
    } else if *topic0 == *APPROVAL_TOPIC0 {
        decode_approval_event(*address, topics, data)?
    } else if *topic0 == *APPROVAL_FOR_ALL_TOPIC0 {
        decode_approval_for_all_event(*address, topics, data)?
    } else if *topic0 == *TRANSFER_SINGLE_TOPIC0 {
        decode_transfer_single_event(*address, topics, data)?
    } else if *topic0 == *TRANSFER_BATCH_TOPIC0 {
        decode_transfer_batch_event(*address, topics, data)?
    } else {
        return Ok(None);
    };

    Ok(Some(decoded))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum DecodedEvent {
    Erc20Transfer {
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
    Erc20Approval {
        token: Address,
        owner: Address,
        spender: Address,
        value: U256,
    },
    Erc721Approval {
        collection: Address,
        owner: Address,
        approved_address: Address,
        token_id: U256,
    },
    OperatorApproval {
        collection: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
    Erc1155TransferSingle {
        collection: Address,
        from: Address,
        to: Address,
        token_id: U256,
        amount: U256,
    },
    Erc1155TransferBatch {
        collection: Address,
        from: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc1155TransferItem {
    pub(super) token_id: U256,
    pub(super) amount: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportedEvent {
    Transfer,
    Approval,
    ApprovalForAll,
    TransferSingle,
    TransferBatch,
}

impl fmt::Display for SupportedEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Transfer => "Transfer",
            Self::Approval => "Approval",
            Self::ApprovalForAll => "ApprovalForAll",
            Self::TransferSingle => "TransferSingle",
            Self::TransferBatch => "TransferBatch",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("malformed {event} event: {reason}")]
pub(crate) struct EventCodecError {
    pub(super) event: SupportedEvent,
    pub(super) reason: &'static str,
}

impl EventCodecError {
    pub(super) const fn malformed(event: SupportedEvent, reason: &'static str) -> Self {
        Self { event, reason }
    }
}

fn decode_transfer_event(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
) -> Result<DecodedEvent, EventCodecError> {
    let event = SupportedEvent::Transfer;

    match (topics.len(), data.len()) {
        (3, 32) => Ok(DecodedEvent::Erc20Transfer {
            token: contract_address,
            from: indexed_address(&topics[1], event)?,
            to: indexed_address(&topics[2], event)?,
            amount: U256::from_be_slice(data),
        }),
        (4, 0) => Ok(DecodedEvent::Erc721Transfer {
            collection: contract_address,
            from: indexed_address(&topics[1], event)?,
            to: indexed_address(&topics[2], event)?,
            token_id: U256::from_be_slice(topics[3].as_slice()),
        }),
        _ => Err(EventCodecError::malformed(
            event,
            "expected ERC-20 or ERC-721 Transfer shape",
        )),
    }
}

fn indexed_address(topic: &B256, event: SupportedEvent) -> Result<Address, EventCodecError> {
    if topic.as_slice()[..12].iter().any(|byte| *byte != 0) {
        return Err(EventCodecError::malformed(
            event,
            "indexed address is not zero padded",
        ));
    }

    Ok(Address::from_word(*topic))
}

fn decode_approval_event(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
) -> Result<DecodedEvent, EventCodecError> {
    let event = SupportedEvent::Approval;

    match (topics.len(), data.len()) {
        (3, 32) => Ok(DecodedEvent::Erc20Approval {
            token: contract_address,
            owner: indexed_address(&topics[1], event)?,
            spender: indexed_address(&topics[2], event)?,
            value: U256::from_be_slice(data),
        }),
        (4, 0) => Ok(DecodedEvent::Erc721Approval {
            collection: contract_address,
            owner: indexed_address(&topics[1], event)?,
            approved_address: indexed_address(&topics[2], event)?,
            token_id: U256::from_be_slice(topics[3].as_slice()),
        }),
        _ => Err(EventCodecError::malformed(
            event,
            "expected ERC-20 or ERC-721 Approval shape",
        )),
    }
}

fn decode_approval_for_all_event(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
) -> Result<DecodedEvent, EventCodecError> {
    let event = SupportedEvent::ApprovalForAll;

    if topics.len() != 3 || data.len() != 32 {
        return Err(EventCodecError::malformed(
            event,
            "expected 3 topics and 32 data bytes",
        ));
    }

    let approved = match U256::from_be_slice(data) {
        value if value.is_zero() => false,
        value if value == U256::from(1_u8) => true,
        _ => {
            return Err(EventCodecError::malformed(
                event,
                "approved value is not a canonical bool",
            ));
        }
    };

    Ok(DecodedEvent::OperatorApproval {
        collection: contract_address,
        owner: indexed_address(&topics[1], event)?,
        operator: indexed_address(&topics[2], event)?,
        approved,
    })
}

fn decode_transfer_single_event(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
) -> Result<DecodedEvent, EventCodecError> {
    let event = SupportedEvent::TransferSingle;

    if topics.len() != 4 {
        return Err(EventCodecError::malformed(event, "expected 4 topics"));
    }

    // Operator is not needed by the movement candidate, but its encoding
    // still belongs to the supported event shape and must be valid.
    indexed_address(&topics[1], event)?;
    let from = indexed_address(&topics[2], event)?;
    let to = indexed_address(&topics[3], event)?;

    let (token_id, amount) = <(U256, U256)>::abi_decode_sequence_validate(data).map_err(|_| {
        EventCodecError::malformed(event, "data is not a canonical (uint256,uint256) tuple")
    })?;

    Ok(DecodedEvent::Erc1155TransferSingle {
        collection: contract_address,
        from,
        to,
        token_id,
        amount,
    })
}

fn decode_transfer_batch_event(
    contract_address: Address,
    topics: &[B256],
    data: &[u8],
) -> Result<DecodedEvent, EventCodecError> {
    let event = SupportedEvent::TransferBatch;

    if topics.len() != 4 {
        return Err(EventCodecError::malformed(event, "expected 4 topics"));
    }

    indexed_address(&topics[1], event)?;
    let from = indexed_address(&topics[2], event)?;
    let to = indexed_address(&topics[3], event)?;

    let (token_ids, amounts) = <(Vec<U256>, Vec<U256>)>::abi_decode_sequence_validate(data)
        .map_err(|_| {
            EventCodecError::malformed(event, "data is not a canonical (uint256[],uint256[]) tuple")
        })?;

    if token_ids.len() != amounts.len() {
        return Err(EventCodecError::malformed(
            event,
            "token ID and amount arrays have different lengths",
        ));
    }

    let items = token_ids
        .into_iter()
        .zip(amounts)
        .map(|(token_id, amount)| Erc1155TransferItem { token_id, amount })
        .collect();

    Ok(DecodedEvent::Erc1155TransferBatch {
        collection: contract_address,
        from,
        to,
        items,
    })
}

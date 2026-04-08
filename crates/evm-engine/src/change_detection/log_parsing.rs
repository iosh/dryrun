use std::sync::LazyLock;

use alloy::sol_types::SolValue;
use alloy_primitives::{Address, B256, U256, keccak256};

use crate::change_observation::Observation;

// This module only validates and decodes well-known log shapes. Semantic
// interpretation stays in the detectors.
static TRANSFER_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Transfer(address,address,uint256)".as_bytes()));
static APPROVAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Approval(address,address,uint256)".as_bytes()));
static APPROVAL_FOR_ALL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("ApprovalForAll(address,address,bool)".as_bytes()));
static TRANSFER_SINGLE_TOPIC0: LazyLock<B256> = LazyLock::new(|| {
    keccak256("TransferSingle(address,address,address,uint256,uint256)".as_bytes())
});
static TRANSFER_BATCH_TOPIC0: LazyLock<B256> = LazyLock::new(|| {
    keccak256("TransferBatch(address,address,address,uint256[],uint256[])".as_bytes())
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc20TransferLog {
    pub(super) contract_address: Address,
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc721TransferLog {
    pub(super) contract_address: Address,
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc20ApprovalLog {
    pub(super) contract_address: Address,
    pub(super) owner: Address,
    pub(super) spender: Address,
    pub(super) value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc721ApprovalLog {
    pub(super) contract_address: Address,
    pub(super) owner: Address,
    pub(super) spender: Address,
    pub(super) token_id: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ApprovalForAllLog {
    pub(super) contract_address: Address,
    pub(super) owner: Address,
    pub(super) operator: Address,
    pub(super) approved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc1155TransferSingleLog {
    pub(super) contract_address: Address,
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) id: U256,
    pub(super) value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Erc1155BatchTransferEntry {
    pub(super) id: U256,
    pub(super) value: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Erc1155TransferBatchLog {
    pub(super) contract_address: Address,
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) transfers: Vec<Erc1155BatchTransferEntry>,
}

#[cfg(test)]
pub(super) fn transfer_topic0() -> B256 {
    *TRANSFER_TOPIC0
}

#[cfg(test)]
pub(super) fn approval_topic0() -> B256 {
    *APPROVAL_TOPIC0
}

#[cfg(test)]
pub(super) fn approval_for_all_topic0() -> B256 {
    *APPROVAL_FOR_ALL_TOPIC0
}

#[cfg(test)]
pub(super) fn transfer_single_topic0() -> B256 {
    *TRANSFER_SINGLE_TOPIC0
}

#[cfg(test)]
pub(super) fn transfer_batch_topic0() -> B256 {
    *TRANSFER_BATCH_TOPIC0
}

fn log_parts(observation: &Observation) -> Option<(Address, &[B256], &[u8])> {
    let Observation::Log {
        address,
        topics,
        data,
    } = observation
    else {
        return None;
    };

    Some((*address, topics.as_slice(), data.as_ref()))
}

pub(super) fn extract_erc20_transfer_log(observation: &Observation) -> Option<Erc20TransferLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *TRANSFER_TOPIC0 || data.len() != 32 {
        return None;
    }

    Some(Erc20TransferLog {
        contract_address,
        from: topic_address(&topics[1])?,
        to: topic_address(&topics[2])?,
        value: U256::from_be_slice(data),
    })
}

pub(super) fn extract_erc721_transfer_log(observation: &Observation) -> Option<Erc721TransferLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 4 || topics[0] != *TRANSFER_TOPIC0 || !data.is_empty() {
        return None;
    }

    Some(Erc721TransferLog {
        contract_address,
        from: topic_address(&topics[1])?,
        to: topic_address(&topics[2])?,
        token_id: topic_u256(&topics[3]),
    })
}

pub(super) fn extract_erc20_approval_log(observation: &Observation) -> Option<Erc20ApprovalLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *APPROVAL_TOPIC0 || data.len() != 32 {
        return None;
    }

    Some(Erc20ApprovalLog {
        contract_address,
        owner: topic_address(&topics[1])?,
        spender: topic_address(&topics[2])?,
        value: U256::from_be_slice(data),
    })
}

pub(super) fn extract_erc721_approval_log(observation: &Observation) -> Option<Erc721ApprovalLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 4 || topics[0] != *APPROVAL_TOPIC0 || !data.is_empty() {
        return None;
    }

    Some(Erc721ApprovalLog {
        contract_address,
        owner: topic_address(&topics[1])?,
        spender: topic_address(&topics[2])?,
        token_id: topic_u256(&topics[3]),
    })
}

pub(super) fn extract_approval_for_all_log(observation: &Observation) -> Option<ApprovalForAllLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *APPROVAL_FOR_ALL_TOPIC0 || data.len() != 32 {
        return None;
    }

    let approved_word = U256::from_be_slice(data);
    let approved = match approved_word {
        value if value.is_zero() => false,
        value if value == U256::from(1_u8) => true,
        _ => return None,
    };

    Some(ApprovalForAllLog {
        contract_address,
        owner: topic_address(&topics[1])?,
        operator: topic_address(&topics[2])?,
        approved,
    })
}

pub(super) fn extract_erc1155_transfer_single_log(
    observation: &Observation,
) -> Option<Erc1155TransferSingleLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 4 || topics[0] != *TRANSFER_SINGLE_TOPIC0 {
        return None;
    }

    let (_operator, from, to) = transfer_operator_and_parties(topics)?;
    let (id, value) = <(U256, U256)>::abi_decode_sequence_validate(data).ok()?;

    Some(Erc1155TransferSingleLog {
        contract_address,
        from,
        to,
        id,
        value,
    })
}

pub(super) fn extract_erc1155_transfer_batch_log(
    observation: &Observation,
) -> Option<Erc1155TransferBatchLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 4 || topics[0] != *TRANSFER_BATCH_TOPIC0 {
        return None;
    }

    let (_operator, from, to) = transfer_operator_and_parties(topics)?;
    let (ids, values) = <(Vec<U256>, Vec<U256>)>::abi_decode_sequence_validate(data).ok()?;

    if ids.len() != values.len() {
        return None;
    }

    Some(Erc1155TransferBatchLog {
        contract_address,
        from,
        to,
        transfers: ids
            .into_iter()
            .zip(values)
            .map(|(id, value)| Erc1155BatchTransferEntry { id, value })
            .collect(),
    })
}

fn transfer_operator_and_parties(topics: &[B256]) -> Option<(Address, Address, Address)> {
    Some((
        topic_address(&topics[1])?,
        topic_address(&topics[2])?,
        topic_address(&topics[3])?,
    ))
}

fn topic_address(topic: &B256) -> Option<Address> {
    is_zero_padded_address_topic(topic).then(|| Address::from_word(*topic))
}

fn topic_u256(topic: &B256) -> U256 {
    U256::from_be_slice(topic.as_slice())
}

fn is_zero_padded_address_topic(topic: &B256) -> bool {
    topic.as_slice()[..12].iter().all(|&byte| byte == 0)
}

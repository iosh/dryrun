use std::sync::LazyLock;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TransferLog {
    pub(super) contract_address: Address,
    pub(super) from: Address,
    pub(super) to: Address,
    pub(super) value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ApprovalLog {
    pub(super) contract_address: Address,
    pub(super) owner: Address,
    pub(super) spender: Address,
    pub(super) value: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ApprovalForAllLog {
    pub(super) contract_address: Address,
    pub(super) owner: Address,
    pub(super) operator: Address,
    pub(super) approved: bool,
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

pub(super) fn extract_transfer_log(observation: &Observation) -> Option<TransferLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *TRANSFER_TOPIC0 || data.len() != 32 {
        return None;
    }

    if !is_zero_padded_address_topic(&topics[1]) || !is_zero_padded_address_topic(&topics[2]) {
        return None;
    }

    Some(TransferLog {
        contract_address,
        from: Address::from_word(topics[1]),
        to: Address::from_word(topics[2]),
        value: U256::from_be_slice(data),
    })
}

pub(super) fn extract_approval_log(observation: &Observation) -> Option<ApprovalLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *APPROVAL_TOPIC0 || data.len() != 32 {
        return None;
    }

    if !is_zero_padded_address_topic(&topics[1]) || !is_zero_padded_address_topic(&topics[2]) {
        return None;
    }

    Some(ApprovalLog {
        contract_address,
        owner: Address::from_word(topics[1]),
        spender: Address::from_word(topics[2]),
        value: U256::from_be_slice(data),
    })
}

pub(super) fn extract_approval_for_all_log(observation: &Observation) -> Option<ApprovalForAllLog> {
    let (contract_address, topics, data) = log_parts(observation)?;

    if topics.len() != 3 || topics[0] != *APPROVAL_FOR_ALL_TOPIC0 || data.len() != 32 {
        return None;
    }

    if !is_zero_padded_address_topic(&topics[1]) || !is_zero_padded_address_topic(&topics[2]) {
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
        owner: Address::from_word(topics[1]),
        operator: Address::from_word(topics[2]),
        approved,
    })
}

fn is_zero_padded_address_topic(topic: &B256) -> bool {
    topic.as_slice()[..12].iter().all(|&byte| byte == 0)
}

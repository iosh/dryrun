use std::fmt;

use alloy_primitives::{Address as AlloyAddress, B256, Bytes as AlloyBytes, U256 as AlloyU256};
use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransactionInputField {
    ChainId,
    StorageLimit,
    EpochHeight,
}

impl fmt::Display for TransactionInputField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChainId => "chain_id",
            Self::StorageLimit => "storage_limit",
            Self::EpochHeight => "epoch_height",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("transaction field `{field}` must fit into an unsigned {max_bits}-bit integer")]
pub struct TransactionInputError {
    field: TransactionInputField,
    max_bits: u16,
}

impl TransactionInputError {
    pub(crate) fn out_of_range(field: TransactionInputField, max_bits: u16) -> Self {
        Self { field, max_bits }
    }
}

pub(crate) fn to_cfx_address(value: AlloyAddress) -> Address {
    Address::from_slice(value.as_slice())
}

pub(crate) fn to_cfx_h256(value: B256) -> H256 {
    H256::from_slice(value.as_slice())
}

pub(crate) fn to_cfx_u256(value: AlloyU256) -> U256 {
    U256::from_big_endian(&value.to_be_bytes::<32>())
}

pub(crate) fn to_cfx_bytes(value: AlloyBytes) -> Bytes {
    value.to_vec()
}

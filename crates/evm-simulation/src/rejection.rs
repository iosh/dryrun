use std::fmt;

use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvmTransactionRejection {
    PriorityFeeGreaterThanMaxFee {
        max_priority_fee_per_gas: u128,
        max_fee_per_gas: u128,
    },
    GasPriceBelowBaseFee {
        gas_price: u128,
        base_fee_per_gas: u64,
    },
    GasLimitExceedsBlockGasLimit {
        gas_limit: u64,
        block_gas_limit: u64,
    },
    GasLimitExceedsCap {
        gas_limit: u64,
        cap: u64,
    },
    IntrinsicGasExceedsGasLimit {
        intrinsic_gas: u64,
        gas_limit: u64,
    },
    FloorGasExceedsGasLimit {
        floor_gas: u64,
        gas_limit: u64,
    },
    SenderHasCode {
        sender: Address,
    },
    InsufficientFunds {
        required: U256,
        balance: U256,
    },
    PaymentOverflow,
    NonceOverflow,
    NonceTooHigh {
        transaction_nonce: u64,
        state_nonce: u64,
    },
    NonceTooLow {
        transaction_nonce: u64,
        state_nonce: u64,
    },
    CreateInitCodeSizeLimit,
    InvalidChainId {
        transaction_chain_id: u64,
        expected_chain_id: u64,
    },
    BlobGasPriceExceedsMaxFee {
        blob_gas_price: u128,
        max_fee_per_blob_gas: u128,
    },
    BlobCountExceedsLimit {
        blob_count: usize,
        max_blob_count: usize,
    },
    UnsupportedBlobVersion {
        blob_index: usize,
        version: u8,
    },
    Eip2930NotActivated,
    Eip1559NotActivated,
    Eip4844NotActivated,
    Eip7702NotActivated,
}

impl fmt::Display for EvmTransactionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PriorityFeeGreaterThanMaxFee {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            } => write!(
                formatter,
                "max priority fee per gas {max_priority_fee_per_gas} exceeds max fee per gas {max_fee_per_gas}"
            ),
            Self::GasPriceBelowBaseFee {
                gas_price,
                base_fee_per_gas,
            } => write!(
                formatter,
                "gas price {gas_price} is below block base fee {base_fee_per_gas}"
            ),
            Self::GasLimitExceedsBlockGasLimit {
                gas_limit,
                block_gas_limit,
            } => write!(
                formatter,
                "transaction gas limit {gas_limit} exceeds block gas limit {block_gas_limit}"
            ),
            Self::GasLimitExceedsCap { gas_limit, cap } => write!(
                formatter,
                "transaction gas limit {gas_limit} exceeds configured cap {cap}"
            ),
            Self::IntrinsicGasExceedsGasLimit {
                intrinsic_gas,
                gas_limit,
            } => write!(
                formatter,
                "intrinsic gas {intrinsic_gas} exceeds transaction gas limit {gas_limit}"
            ),
            Self::FloorGasExceedsGasLimit {
                floor_gas,
                gas_limit,
            } => write!(
                formatter,
                "floor gas {floor_gas} exceeds transaction gas limit {gas_limit}"
            ),
            Self::SenderHasCode { sender } => {
                write!(formatter, "sender {sender} has deployed code")
            }
            Self::InsufficientFunds { required, balance } => {
                write!(
                    formatter,
                    "insufficient funds: required {required}, available balance {balance}"
                )
            }
            Self::PaymentOverflow => {
                formatter.write_str("transaction payment calculation overflowed")
            }
            Self::NonceOverflow => formatter.write_str("transaction nonce overflowed"),
            Self::NonceTooHigh {
                transaction_nonce,
                state_nonce,
            } => write!(
                formatter,
                "nonce {transaction_nonce} too high, expected {state_nonce}"
            ),
            Self::NonceTooLow {
                transaction_nonce,
                state_nonce,
            } => write!(
                formatter,
                "nonce {transaction_nonce} too low, expected {state_nonce}"
            ),
            Self::CreateInitCodeSizeLimit => {
                formatter.write_str("contract creation initcode exceeds the protocol size limit")
            }
            Self::InvalidChainId {
                transaction_chain_id,
                expected_chain_id,
            } => write!(
                formatter,
                "transaction chain ID {transaction_chain_id} does not match expected chain ID {expected_chain_id}"
            ),
            Self::BlobGasPriceExceedsMaxFee {
                blob_gas_price,
                max_fee_per_blob_gas,
            } => write!(
                formatter,
                "block blob gas price {blob_gas_price} exceeds max fee per blob gas {max_fee_per_blob_gas}"
            ),
            Self::BlobCountExceedsLimit {
                blob_count,
                max_blob_count,
            } => write!(
                formatter,
                "transaction contains {blob_count} blobs, exceeding the limit of {max_blob_count}"
            ),
            Self::UnsupportedBlobVersion {
                blob_index,
                version,
            } => write!(
                formatter,
                "blob versioned hash at index {blob_index} uses unsupported version 0x{version:02x}"
            ),
            Self::Eip2930NotActivated => {
                formatter.write_str("EIP-2930 is not active at the selected block")
            }
            Self::Eip1559NotActivated => {
                formatter.write_str("EIP-1559 is not active at the selected block")
            }
            Self::Eip4844NotActivated => {
                formatter.write_str("EIP-4844 is not active at the selected block")
            }
            Self::Eip7702NotActivated => {
                formatter.write_str("EIP-7702 is not active at the selected block")
            }
        }
    }
}

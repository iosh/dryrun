use std::fmt;

use alloy_primitives::{Address, U256, U512};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EspaceTransactionRejection {
    InvalidChainId {
        transaction_chain_id: u64,
        expected_chain_id: u64,
    },
    ZeroGasPrice,
    PriorityFeeGreaterThanMaxFee {
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    },
    LegacyTransactionNotActivated,
    Eip2930NotActivated,
    Eip1559NotActivated,
    Eip7702NotActivated,
    CreateInitCodeSizeLimit {
        size: usize,
        limit: usize,
    },
    CalldataGasRequirement {
        required_gas: U256,
        gas_limit: u64,
    },
    NonceTooLow {
        transaction_nonce: U256,
        state_nonce: U256,
    },
    NonceTooHigh {
        transaction_nonce: U256,
        state_nonce: U256,
    },
    IntrinsicGasExceedsGasLimit {
        intrinsic_gas: U256,
        gas_limit: U256,
    },
    SenderHasCode {
        sender: Address,
    },
    SenderDoesNotExist,
    GasPriceBelowBaseFee {
        gas_price: U256,
        base_fee_per_gas: U256,
    },
    InsufficientFunds {
        required: U512,
        available: U512,
    },
}

impl fmt::Display for EspaceTransactionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChainId {
                transaction_chain_id,
                expected_chain_id,
            } => write!(
                formatter,
                "transaction chain id {transaction_chain_id} does not match eSpace chain id {expected_chain_id}"
            ),
            Self::ZeroGasPrice => {
                formatter.write_str("transaction gas price must be greater than zero")
            }
            Self::PriorityFeeGreaterThanMaxFee {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            } => write!(
                formatter,
                "max priority fee per gas {max_priority_fee_per_gas} exceeds max fee per gas {max_fee_per_gas}"
            ),
            Self::LegacyTransactionNotActivated => formatter
                .write_str("legacy eSpace transactions are not active at the selected block"),
            Self::Eip2930NotActivated => {
                formatter.write_str("EIP-2930 transactions are not active at the selected block")
            }
            Self::Eip1559NotActivated => {
                formatter.write_str("EIP-1559 transactions are not active at the selected block")
            }
            Self::Eip7702NotActivated => {
                formatter.write_str("EIP-7702 transactions are not active at the selected block")
            }
            Self::CreateInitCodeSizeLimit { size, limit } => write!(
                formatter,
                "contract initcode size {size} exceeds the protocol limit {limit}"
            ),
            Self::CalldataGasRequirement {
                required_gas,
                gas_limit,
            } => write!(
                formatter,
                "transaction gas limit {gas_limit} is lower than the calldata requirement {required_gas}"
            ),
            Self::NonceTooLow {
                transaction_nonce,
                state_nonce,
            } => write!(
                formatter,
                "transaction nonce {transaction_nonce} is lower than state nonce {state_nonce}"
            ),
            Self::NonceTooHigh {
                transaction_nonce,
                state_nonce,
            } => write!(
                formatter,
                "transaction nonce {transaction_nonce} is higher than state nonce {state_nonce}"
            ),
            Self::IntrinsicGasExceedsGasLimit {
                intrinsic_gas,
                gas_limit,
            } => write!(
                formatter,
                "transaction gas limit {gas_limit} is lower than intrinsic gas {intrinsic_gas}"
            ),
            Self::SenderHasCode { sender } => {
                write!(formatter, "transaction sender {sender} has contract code")
            }
            Self::SenderDoesNotExist => formatter.write_str("transaction sender does not exist"),
            Self::GasPriceBelowBaseFee {
                gas_price,
                base_fee_per_gas,
            } => write!(
                formatter,
                "transaction gas price {gas_price} is lower than base fee {base_fee_per_gas}"
            ),
            Self::InsufficientFunds {
                required,
                available,
            } => write!(
                formatter,
                "sender balance {available} is lower than required cost {required}"
            ),
        }
    }
}

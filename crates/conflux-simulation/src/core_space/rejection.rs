use std::fmt;

use alloy_primitives::{U256, U512};
use conflux_provider::CoreAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreSpaceTransactionRejection {
    InvalidChainId {
        transaction_chain_id: u32,
        expected_chain_id: u32,
    },
    ZeroGasPrice,
    ZeroMaxFeePerGas,
    PriorityFeeGreaterThanMaxFee {
        max_priority_fee_per_gas: U256,
        max_fee_per_gas: U256,
    },
    Cip2930NotActivated,
    Cip1559NotActivated,
    NonceTooLow {
        transaction_nonce: U256,
        state_nonce: U256,
    },
    NonceTooHigh {
        transaction_nonce: U256,
        state_nonce: U256,
    },
    EpochHeightOutOfBounds {
        execution_epoch_height: u64,
        transaction_epoch_height: u64,
        epoch_bound: u64,
    },
    IntrinsicGasExceedsGasLimit {
        intrinsic_gas: U256,
        gas_limit: U256,
    },
    InvalidRecipient {
        recipient: CoreAddress,
    },
    SenderHasCode {
        sender: CoreAddress,
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
    SponsorBalanceInsufficient {
        required_gas_cost: U512,
        available_gas_balance: U512,
        required_storage_cost: U256,
        available_storage_balance: U256,
    },
}

impl fmt::Display for CoreSpaceTransactionRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChainId {
                transaction_chain_id,
                expected_chain_id,
            } => write!(
                formatter,
                "transaction chain id {transaction_chain_id} does not match Core Space chain id {expected_chain_id}"
            ),
            Self::ZeroGasPrice => {
                formatter.write_str("transaction gas price must be greater than zero")
            }
            Self::ZeroMaxFeePerGas => {
                formatter.write_str("transaction max fee per gas must be greater than zero")
            }
            Self::PriorityFeeGreaterThanMaxFee {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            } => write!(
                formatter,
                "max priority fee per gas {max_priority_fee_per_gas} exceeds max fee per gas {max_fee_per_gas}"
            ),
            Self::Cip2930NotActivated => formatter.write_str(
                "CIP-2930 transactions are not active in the selected Core Space context",
            ),
            Self::Cip1559NotActivated => formatter.write_str(
                "CIP-1559 transactions are not active in the selected Core Space context",
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
            Self::EpochHeightOutOfBounds {
                execution_epoch_height,
                transaction_epoch_height,
                epoch_bound,
            } => write!(
                formatter,
                "transaction epoch height {transaction_epoch_height} is outside execution epoch {execution_epoch_height} bound {epoch_bound}"
            ),
            Self::IntrinsicGasExceedsGasLimit {
                intrinsic_gas,
                gas_limit,
            } => write!(
                formatter,
                "transaction gas limit {gas_limit} is lower than intrinsic gas {intrinsic_gas}"
            ),
            Self::InvalidRecipient { recipient } => {
                write!(
                    formatter,
                    "invalid Core Space recipient address {recipient}"
                )
            }
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
            Self::SponsorBalanceInsufficient {
                required_gas_cost,
                available_gas_balance,
                required_storage_cost,
                available_storage_balance,
            } => write!(
                formatter,
                "sponsor balance is insufficient: required gas {required_gas_cost}, available gas {available_gas_balance}, required storage {required_storage_cost}, available storage {available_storage_balance}"
            ),
        }
    }
}

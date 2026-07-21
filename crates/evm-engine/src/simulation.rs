use alloy_primitives::{B256, Bytes, U256};

use crate::Change;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionFailure {
    pub code: EvmExecutionFailureCode,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmExecutionFailureCode {
    Revert,
    OutOfGas,
    InvalidOpcode,
    InvalidJump,
    StackUnderflow,
    StackOverflow,
    ExecutionFailed,
    NonceTooLow,
    NonceTooHigh,
    NonceOverflow,
    InsufficientFunds,
    PriorityFeeGreaterThanMaxFee,
    GasPriceLessThanBaseFee,
    GasLimitExceedsBlockGasLimit,
    IntrinsicGasTooLow,
    SenderHasCode,
    InvalidChainId,
    TransactionTypeNotSupported,
    InvalidTransaction,
}

impl EvmExecutionFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Revert => "REVERT",
            Self::OutOfGas => "OUT_OF_GAS",
            Self::InvalidOpcode => "INVALID_OPCODE",
            Self::InvalidJump => "INVALID_JUMP",
            Self::StackUnderflow => "STACK_UNDERFLOW",
            Self::StackOverflow => "STACK_OVERFLOW",
            Self::ExecutionFailed => "EXECUTION_FAILED",
            Self::NonceTooLow => "NONCE_TOO_LOW",
            Self::NonceTooHigh => "NONCE_TOO_HIGH",
            Self::NonceOverflow => "NONCE_OVERFLOW",
            Self::InsufficientFunds => "INSUFFICIENT_FUNDS",
            Self::PriorityFeeGreaterThanMaxFee => "PRIORITY_FEE_GREATER_THAN_MAX_FEE",
            Self::GasPriceLessThanBaseFee => "GAS_PRICE_LESS_THAN_BASE_FEE",
            Self::GasLimitExceedsBlockGasLimit => "GAS_LIMIT_EXCEEDS_BLOCK_GAS_LIMIT",
            Self::IntrinsicGasTooLow => "INTRINSIC_GAS_TOO_LOW",
            Self::SenderHasCode => "SENDER_HAS_CODE",
            Self::InvalidChainId => "INVALID_CHAIN_ID",
            Self::TransactionTypeNotSupported => "TRANSACTION_TYPE_NOT_SUPPORTED",
            Self::InvalidTransaction => "INVALID_TRANSACTION",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmExecutionOutcome {
    Success {
        gas_used: u64,
        fee: U256,
        burnt_fee: U256,
        output: Bytes,
    },
    Failed {
        gas_used: u64,
        fee: U256,
        burnt_fee: U256,
        output: Bytes,
        failure: EvmExecutionFailure,
    },
    NotExecuted {
        failure: EvmExecutionFailure,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub gas_limit: u64,
    pub outcome: EvmExecutionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulation {
    pub execution: EvmExecution,
    pub changes: Vec<Change>,
}

impl EvmSimulation {
    pub fn new(execution: EvmExecution, changes: Vec<Change>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &EvmExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn into_parts(self) -> (EvmExecution, Vec<Change>) {
        (self.execution, self.changes)
    }
}

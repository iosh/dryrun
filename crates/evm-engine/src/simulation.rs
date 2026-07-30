use crate::Change;
use simulation_execution::Execution;
pub use simulation_execution::{ExecutedDetails, ExecutionOutcome, SimulatedBlock};

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

pub type EvmExecution = Execution<SimulatedBlock, ExecutedDetails, EvmExecutionFailure>;
pub type EvmExecutionOutcome = ExecutionOutcome<ExecutedDetails, EvmExecutionFailure>;

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

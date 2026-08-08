use std::fmt;

use alloy::sol_types::Panic;
use alloy_primitives::{Address, Bytes, Log, U256};

use crate::{EvmExecutionResult, EvmTransactionRejection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmSuccessReason {
    Stop,
    Return,
    SelfDestruct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmSuccessOutput {
    Call {
        return_data: Bytes,
    },
    Create {
        address: Address,
        runtime_code: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvmRevertReason {
    SolidityError { message: String },
    SolidityPanic { code: U256 },
}

impl fmt::Display for EvmRevertReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SolidityError { message } if message.is_empty() => formatter.write_str("<empty>"),
            Self::SolidityError { message } => formatter.write_str(message),
            Self::SolidityPanic { code } => {
                formatter.write_str(Panic { code: *code }.as_geth_str().as_ref())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvmOutOfGasReason {
    Basic,
    MemoryLimit,
    MemoryExpansion,
    Precompile,
    InvalidOperand,
    ReentrancySentry,
}

impl fmt::Display for EvmOutOfGasReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Basic => "out of gas",
            Self::MemoryLimit => "out of gas: memory limit exceeded",
            Self::MemoryExpansion => "out of gas: memory expansion",
            Self::Precompile => "out of gas: precompile",
            Self::InvalidOperand => "out of gas: invalid operand",
            Self::ReentrancySentry => "out of gas: reentrancy sentry",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvmHaltReason {
    OutOfGas(EvmOutOfGasReason),
    OpcodeNotFound,
    InvalidFeOpcode,
    InvalidJump,
    NotActivated,
    StackUnderflow,
    StackOverflow,
    OutOfOffset,
    CreateCollision,
    PrecompileError,
    PrecompileErrorWithContext { message: String },
    NonceOverflow,
    CreateContractSizeLimit,
    CreateContractStartingWithEf,
    CreateInitCodeSizeLimit,
    PaymentOverflow,
    StateChangeDuringStaticCall,
    CallNotAllowedInsideStatic,
    OutOfFunds,
    CallTooDeep,
}

impl fmt::Display for EvmHaltReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfGas(reason) => reason.fmt(formatter),
            Self::OpcodeNotFound => formatter.write_str("opcode not found"),
            Self::InvalidFeOpcode => formatter.write_str("invalid 0xFE opcode"),
            Self::InvalidJump => formatter.write_str("invalid jump destination"),
            Self::NotActivated => formatter.write_str("opcode is not active at the selected block"),
            Self::StackUnderflow => formatter.write_str("stack underflow"),
            Self::StackOverflow => formatter.write_str("stack overflow"),
            Self::OutOfOffset => formatter.write_str("out of offset"),
            Self::CreateCollision => formatter.write_str("create collision"),
            Self::PrecompileError => formatter.write_str("precompile error"),
            Self::PrecompileErrorWithContext { message } => {
                write!(formatter, "precompile error: {message}")
            }
            Self::NonceOverflow => formatter.write_str("nonce overflow"),
            Self::CreateContractSizeLimit => {
                formatter.write_str("contract runtime code exceeds the protocol size limit")
            }
            Self::CreateContractStartingWithEf => {
                formatter.write_str("contract runtime code starts with the forbidden 0xEF byte")
            }
            Self::CreateInitCodeSizeLimit => {
                formatter.write_str("contract initcode exceeds the protocol size limit")
            }
            Self::PaymentOverflow => formatter.write_str("payment calculation overflowed"),
            Self::StateChangeDuringStaticCall => {
                formatter.write_str("state change during static call")
            }
            Self::CallNotAllowedInsideStatic => {
                formatter.write_str("call not allowed inside static call")
            }
            Self::OutOfFunds => formatter.write_str("insufficient funds"),
            Self::CallTooDeep => formatter.write_str("call depth limit exceeded"),
        }
    }
}

/// Outcome of attempting to execute a complete transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmExecutionOutcome {
    Success {
        result: EvmExecutionResult,
        reason: EvmSuccessReason,
        output: EvmSuccessOutput,
        logs: Vec<Log>,
    },
    Reverted {
        result: EvmExecutionResult,
        revert_data: Bytes,
        reason: Option<EvmRevertReason>,
    },
    Halted {
        result: EvmExecutionResult,
        reason: EvmHaltReason,
    },
    NotExecuted(EvmTransactionRejection),
}

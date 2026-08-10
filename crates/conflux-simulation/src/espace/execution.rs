use std::fmt;

use alloy::sol_types::Panic;
use alloy_primitives::{Address, B256, Bytes, U256, U512};
use conflux_provider::CoreAddress;

use super::{EspaceExecutionResult, EspaceTransactionRejection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceLogAddress {
    Espace(Address),
    CoreSpace(CoreAddress),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceLog {
    pub address: EspaceLogAddress,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceSuccessOutput {
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
pub enum EspaceRevertReason {
    SolidityError { message: String },
    SolidityPanic { code: U256 },
}

impl fmt::Display for EspaceRevertReason {
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EspaceExecutionFailure {
    InsufficientFunds {
        required: U512,
        available: U512,
        actual_gas_cost: U256,
        maximum_storage_cost: U256,
    },
    OutOfGas,
    InvalidJump {
        destination: usize,
    },
    InvalidInstruction {
        instruction: u8,
    },
    StackUnderflow {
        instruction: &'static str,
        wanted: usize,
        available: usize,
    },
    StackOverflow {
        instruction: &'static str,
        wanted: usize,
        limit: usize,
    },
    SubroutineStackUnderflow {
        wanted: usize,
        available: usize,
    },
    SubroutineStackOverflow {
        wanted: usize,
        limit: usize,
    },
    InvalidSubroutineEntry,
    BuiltInContract {
        details: String,
    },
    InternalContract {
        details: String,
    },
    StateChangeDuringStaticCall,
    CreateInitCodeSizeLimit,
    ReturnDataOutOfBounds,
    InvalidAddress {
        address: Address,
    },
    CreateCollision {
        address: Address,
    },
    NonceOverflow {
        address: Address,
    },
    CreateContractStartingWithEf,
}

impl fmt::Display for EspaceExecutionFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientFunds {
                required,
                available,
                ..
            } => write!(
                formatter,
                "sender balance {available} is lower than required cost {required}"
            ),
            Self::OutOfGas => formatter.write_str("execution ran out of gas"),
            Self::InvalidJump { destination } => {
                write!(formatter, "invalid jump destination 0x{destination:x}")
            }
            Self::InvalidInstruction { instruction } => {
                write!(formatter, "invalid instruction 0x{instruction:02x}")
            }
            Self::StackUnderflow { .. } => formatter.write_str("stack underflow"),
            Self::StackOverflow { .. } => formatter.write_str("stack overflow"),
            Self::SubroutineStackUnderflow { .. } => {
                formatter.write_str("subroutine stack underflow")
            }
            Self::SubroutineStackOverflow { .. } => {
                formatter.write_str("subroutine stack overflow")
            }
            Self::InvalidSubroutineEntry => formatter.write_str("invalid subroutine entry"),
            Self::BuiltInContract { details } => {
                write!(formatter, "built-in contract failed: {details}")
            }
            Self::InternalContract { details } => {
                write!(formatter, "internal contract failed: {details}")
            }
            Self::StateChangeDuringStaticCall => {
                formatter.write_str("state change during static call")
            }
            Self::CreateInitCodeSizeLimit => {
                formatter.write_str("contract initcode exceeds the protocol size limit")
            }
            Self::ReturnDataOutOfBounds => {
                formatter.write_str("return data access is out of bounds")
            }
            Self::InvalidAddress { address } => write!(formatter, "invalid address {address}"),
            Self::CreateCollision { address } => {
                write!(formatter, "contract creation collides with {address}")
            }
            Self::NonceOverflow { address } => write!(formatter, "nonce overflow for {address}"),
            Self::CreateContractStartingWithEf => {
                formatter.write_str("contract runtime code starts with the forbidden 0xEF byte")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceExecutionOutcome {
    Success {
        result: EspaceExecutionResult,
        output: EspaceSuccessOutput,
        logs: Vec<EspaceLog>,
    },
    Reverted {
        result: EspaceExecutionResult,
        revert_data: Bytes,
        reason: Option<EspaceRevertReason>,
    },
    Failed {
        result: EspaceExecutionResult,
        failure: EspaceExecutionFailure,
    },
    NotExecuted(EspaceTransactionRejection),
}

use std::fmt;

use alloy::sol_types::Panic;
use alloy_primitives::{Address, B256, Bytes, U256, U512};
use conflux_provider::CoreAddress;

use super::{CoreSpaceBlockContext, CoreSpaceExecutionResult, CoreSpaceTransactionRejection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceLogAddress {
    CoreSpace(CoreAddress),
    Espace(Address),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceLog {
    pub address: CoreSpaceLogAddress,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceSuccessOutput {
    Call {
        return_data: Bytes,
    },
    Create {
        address: CoreAddress,
        runtime_code: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreSpaceRevertReason {
    SolidityError { message: String },
    SolidityPanic { code: U256 },
}

impl fmt::Display for CoreSpaceRevertReason {
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
pub enum CoreSpaceExecutionFailure {
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
    StorageBalanceInsufficient {
        required: U256,
        available: U256,
    },
    StorageLimitExceeded,
    BuiltInContract {
        details: String,
    },
    InternalContract {
        details: String,
    },
    StateChangeDuringStaticCall,
    CreateInitCodeSizeLimit,
    Wasm {
        details: String,
    },
    ReturnDataOutOfBounds,
    InvalidAddress {
        address: CoreAddress,
    },
    CreateCollision {
        address: CoreAddress,
    },
    NonceOverflow {
        address: CoreAddress,
    },
    CreateContractStartingWithEf,
}

impl fmt::Display for CoreSpaceExecutionFailure {
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
            Self::StorageBalanceInsufficient {
                required,
                available,
            } => write!(
                formatter,
                "storage collateral balance {available} is lower than required amount {required}"
            ),
            Self::StorageLimitExceeded => {
                formatter.write_str("execution exceeded the transaction storage limit")
            }
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
            Self::Wasm { details } => write!(formatter, "Wasm execution failed: {details}"),
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
pub enum CoreSpaceExecutionOutcome {
    Success {
        result: CoreSpaceExecutionResult,
        output: CoreSpaceSuccessOutput,
        logs: Vec<CoreSpaceLog>,
    },
    Reverted {
        result: CoreSpaceExecutionResult,
        revert_data: Bytes,
        reason: Option<CoreSpaceRevertReason>,
    },
    Failed {
        result: CoreSpaceExecutionResult,
        failure: CoreSpaceExecutionFailure,
    },
    NotExecuted(CoreSpaceTransactionRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecution {
    pub chain_id: u64,
    pub context: CoreSpaceBlockContext,
    pub gas_limit: U256,
    pub outcome: CoreSpaceExecutionOutcome,
}

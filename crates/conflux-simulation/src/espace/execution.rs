use alloy_primitives::{Bytes, U256};
use simulation_execution::Outcome;

use super::EspaceBlockContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceExecutionFailureCode {
    ChainIdMismatch,
    ZeroGasPrice,
    PriorityFeeExceedsMaxFee,
    NonceTooLow,
    NonceTooHigh,
    FeeBelowBaseFee,
    IntrinsicGasTooLow,
    SenderWithCode,
    SenderDoesNotExist,
    InsufficientFunds,
    Revert,
    OutOfGas,
    NonceOverflow,
    VmError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecutionFailure {
    pub code: EspaceExecutionFailureCode,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecutionDetails {
    pub gas_used: u64,
    pub gas_charged: u64,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecution {
    pub chain_id: u64,
    pub context: EspaceBlockContext,
    pub gas_limit: u64,
    pub outcome: EspaceOutcome,
}

pub type EspaceOutcome = Outcome<EspaceExecutionDetails, EspaceExecutionFailure>;

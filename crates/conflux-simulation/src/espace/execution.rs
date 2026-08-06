use alloy_primitives::{B256, Bytes, U256};
use simulation_execution::ExecutionOutcome;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

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
pub struct EspaceExecutedDetails {
    pub gas_used: u64,
    pub gas_charged: u64,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecution {
    pub chain_id: u64,
    pub context: SimulatedBlock,
    pub gas_limit: u64,
    pub outcome: EspaceExecutionOutcome,
}

pub type EspaceExecutionOutcome = ExecutionOutcome<EspaceExecutedDetails, EspaceExecutionFailure>;

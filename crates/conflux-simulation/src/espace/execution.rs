use alloy_primitives::U256;
pub use simulation_execution::SimulatedBlock;
use simulation_execution::{ExecutedDetails, Execution, ExecutionOutcome};

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

pub type EspaceExecutedDetails = ExecutedDetails<Option<U256>>;
pub type EspaceExecution = Execution<SimulatedBlock, EspaceExecutedDetails, EspaceExecutionFailure>;
pub type EspaceExecutionOutcome = ExecutionOutcome<EspaceExecutedDetails, EspaceExecutionFailure>;

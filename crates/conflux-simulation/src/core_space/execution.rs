use alloy_primitives::{Bytes, U256};
use simulation_execution::Outcome;

use super::CoreSpaceBlockContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceExecutionFailureCode {
    ChainIdMismatch,
    ZeroGasPrice,
    PriorityFeeExceedsMaxFee,
    NonceTooLow,
    NonceTooHigh,
    EpochHeightOutOfBound,
    FeeBelowBaseFee,
    IntrinsicGasTooLow,
    InvalidRecipient,
    SenderWithCode,
    SenderDoesNotExist,
    InsufficientFunds,
    SponsorBalanceInsufficient,
    Revert,
    OutOfGas,
    StorageBalanceInsufficient,
    StorageLimitExceeded,
    NonceOverflow,
    VmError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecutionFailure {
    pub code: CoreSpaceExecutionFailureCode,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecutionDetails {
    pub gas_used: u64,
    pub gas_charged: u64,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
    pub gas_covered_by_sponsor: bool,
    pub storage_covered_by_sponsor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecution {
    pub chain_id: u64,
    pub context: CoreSpaceBlockContext,
    pub gas_limit: u64,
    pub outcome: CoreSpaceOutcome,
}

pub type CoreSpaceOutcome = Outcome<CoreSpaceExecutionDetails, CoreSpaceExecutionFailure>;

use alloy_primitives::{Bytes, U256};
use cfx_types::H256;
use simulation_execution::ExecutionOutcome;

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
pub struct CoreSpaceStateAnchor {
    pub epoch_number: u64,
    pub pivot_hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecutedDetails {
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
    pub context: CoreSpaceStateAnchor,
    pub gas_limit: u64,
    pub outcome: CoreSpaceExecutionOutcome,
}

pub type CoreSpaceExecutionOutcome =
    ExecutionOutcome<CoreSpaceExecutedDetails, CoreSpaceExecutionFailure>;

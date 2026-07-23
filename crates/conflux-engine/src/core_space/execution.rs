use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreSpaceExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

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
pub struct CoreSpaceStorageChange {
    pub address: Address,
    pub collateral_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecution {
    pub chain_id: u64,
    pub state: CoreSpaceStateAnchor,
    pub status: CoreSpaceExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub gas_covered_by_sponsor: bool,
    pub storage_covered_by_sponsor: bool,
    pub storage_collateralized: Vec<CoreSpaceStorageChange>,
    pub storage_released: Vec<CoreSpaceStorageChange>,
    pub output: Bytes,
    pub failure: Option<CoreSpaceExecutionFailure>,
}

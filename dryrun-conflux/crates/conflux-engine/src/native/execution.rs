use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeExecutionFailureCode {
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
pub struct NativeExecutionFailure {
    pub code: NativeExecutionFailureCode,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStateAnchor {
    pub epoch_number: u64,
    pub pivot_hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeStorageChange {
    pub address: Address,
    pub collateral_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExecution {
    pub chain_id: u64,
    pub state: NativeStateAnchor,
    pub status: NativeExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub gas_covered_by_sponsor: bool,
    pub storage_covered_by_sponsor: bool,
    pub storage_collateralized: Vec<NativeStorageChange>,
    pub storage_released: Vec<NativeStorageChange>,
    pub output: Bytes,
    pub failure: Option<NativeExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSimulation {
    execution: NativeExecution,
}

impl NativeSimulation {
    pub fn new(execution: NativeExecution) -> Self {
        Self { execution }
    }

    pub fn execution(&self) -> &NativeExecution {
        &self.execution
    }

    pub fn into_execution(self) -> NativeExecution {
        self.execution
    }
}

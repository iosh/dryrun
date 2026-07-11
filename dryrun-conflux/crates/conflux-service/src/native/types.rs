use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use conflux_engine as engine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochRef {
    LatestState,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTransactionVariant {
    Cip155 {
        gas_price: U256,
    },
    Cip2930 {
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    Cip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub storage_limit: u64,
    pub epoch_height: u64,
    pub chain_id: u32,
    pub variant: NativeTransactionVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionFailureCode {
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
pub struct ExecutionFailure {
    pub code: ExecutionFailureCode,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateAnchor {
    pub epoch_number: u64,
    pub pivot_hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageChange {
    pub address: Address,
    pub collateral_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationExecution {
    pub chain_id: u64,
    pub state: StateAnchor,
    pub status: ExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub gas_covered_by_sponsor: bool,
    pub storage_covered_by_sponsor: bool,
    pub storage_collateralized: Vec<StorageChange>,
    pub storage_released: Vec<StorageChange>,
    pub output: Bytes,
    pub failure: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateNativeTransactionInput {
    pub epoch: EpochRef,
    pub transaction: NativeTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateNativeTransactionOutput {
    pub execution: SimulationExecution,
}

impl From<SimulateNativeTransactionInput> for engine::SimulateNativeTransactionInput {
    fn from(input: SimulateNativeTransactionInput) -> Self {
        Self {
            epoch: input.epoch.into(),
            transaction: input.transaction.into(),
        }
    }
}

impl From<EpochRef> for engine::NativeEpochRef {
    fn from(epoch: EpochRef) -> Self {
        match epoch {
            EpochRef::LatestState => Self::LatestState,
            EpochRef::Number(number) => Self::Number(number),
        }
    }
}

impl From<NativeTransaction> for engine::NativeTransaction {
    fn from(transaction: NativeTransaction) -> Self {
        Self {
            from: transaction.from,
            to: transaction.to,
            nonce: transaction.nonce,
            gas_limit: transaction.gas_limit,
            value: transaction.value,
            data: transaction.data,
            storage_limit: transaction.storage_limit,
            epoch_height: transaction.epoch_height,
            chain_id: transaction.chain_id,
            variant: transaction.variant.into(),
        }
    }
}

impl From<NativeTransactionVariant> for engine::NativeTransactionVariant {
    fn from(variant: NativeTransactionVariant) -> Self {
        match variant {
            NativeTransactionVariant::Cip155 { gas_price } => Self::Cip155 { gas_price },
            NativeTransactionVariant::Cip2930 {
                gas_price,
                access_list,
            } => Self::Cip2930 {
                gas_price,
                access_list: map_access_list(access_list),
            },
            NativeTransactionVariant::Cip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => Self::Cip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list: map_access_list(access_list),
            },
        }
    }
}

impl From<engine::NativeSimulation> for SimulateNativeTransactionOutput {
    fn from(simulation: engine::NativeSimulation) -> Self {
        Self {
            execution: simulation.into_execution().into(),
        }
    }
}

impl From<engine::NativeExecution> for SimulationExecution {
    fn from(execution: engine::NativeExecution) -> Self {
        Self {
            chain_id: execution.chain_id,
            state: execution.state.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            gas_charged: execution.gas_charged,
            fee: execution.fee,
            burnt_fee: execution.burnt_fee,
            gas_covered_by_sponsor: execution.gas_covered_by_sponsor,
            storage_covered_by_sponsor: execution.storage_covered_by_sponsor,
            storage_collateralized: execution
                .storage_collateralized
                .into_iter()
                .map(Into::into)
                .collect(),
            storage_released: execution
                .storage_released
                .into_iter()
                .map(Into::into)
                .collect(),
            output: execution.output,
            failure: execution.failure.map(Into::into),
        }
    }
}

impl From<engine::NativeStateAnchor> for StateAnchor {
    fn from(state: engine::NativeStateAnchor) -> Self {
        Self {
            epoch_number: state.epoch_number,
            pivot_hash: state.pivot_hash,
        }
    }
}

impl From<engine::NativeStorageChange> for StorageChange {
    fn from(change: engine::NativeStorageChange) -> Self {
        Self {
            address: change.address,
            collateral_units: change.collateral_units,
        }
    }
}

impl From<engine::NativeExecutionStatus> for ExecutionStatus {
    fn from(status: engine::NativeExecutionStatus) -> Self {
        match status {
            engine::NativeExecutionStatus::Success => Self::Success,
            engine::NativeExecutionStatus::Failed => Self::Failed,
            engine::NativeExecutionStatus::NotExecuted => Self::NotExecuted,
        }
    }
}

impl From<engine::NativeExecutionFailure> for ExecutionFailure {
    fn from(failure: engine::NativeExecutionFailure) -> Self {
        Self {
            code: failure.code.into(),
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<engine::NativeExecutionFailureCode> for ExecutionFailureCode {
    fn from(code: engine::NativeExecutionFailureCode) -> Self {
        match code {
            engine::NativeExecutionFailureCode::ChainIdMismatch => Self::ChainIdMismatch,
            engine::NativeExecutionFailureCode::ZeroGasPrice => Self::ZeroGasPrice,
            engine::NativeExecutionFailureCode::PriorityFeeExceedsMaxFee => {
                Self::PriorityFeeExceedsMaxFee
            }
            engine::NativeExecutionFailureCode::NonceTooLow => Self::NonceTooLow,
            engine::NativeExecutionFailureCode::NonceTooHigh => Self::NonceTooHigh,
            engine::NativeExecutionFailureCode::EpochHeightOutOfBound => {
                Self::EpochHeightOutOfBound
            }
            engine::NativeExecutionFailureCode::FeeBelowBaseFee => Self::FeeBelowBaseFee,
            engine::NativeExecutionFailureCode::IntrinsicGasTooLow => Self::IntrinsicGasTooLow,
            engine::NativeExecutionFailureCode::InvalidRecipient => Self::InvalidRecipient,
            engine::NativeExecutionFailureCode::SenderWithCode => Self::SenderWithCode,
            engine::NativeExecutionFailureCode::SenderDoesNotExist => Self::SenderDoesNotExist,
            engine::NativeExecutionFailureCode::InsufficientFunds => Self::InsufficientFunds,
            engine::NativeExecutionFailureCode::SponsorBalanceInsufficient => {
                Self::SponsorBalanceInsufficient
            }
            engine::NativeExecutionFailureCode::Revert => Self::Revert,
            engine::NativeExecutionFailureCode::OutOfGas => Self::OutOfGas,
            engine::NativeExecutionFailureCode::StorageBalanceInsufficient => {
                Self::StorageBalanceInsufficient
            }
            engine::NativeExecutionFailureCode::StorageLimitExceeded => Self::StorageLimitExceeded,
            engine::NativeExecutionFailureCode::NonceOverflow => Self::NonceOverflow,
            engine::NativeExecutionFailureCode::VmError => Self::VmError,
        }
    }
}

fn map_access_list(items: Vec<AccessListItem>) -> Vec<engine::AccessListItem> {
    items
        .into_iter()
        .map(|item| engine::AccessListItem {
            address: item.address,
            storage_keys: item.storage_keys,
        })
        .collect()
}

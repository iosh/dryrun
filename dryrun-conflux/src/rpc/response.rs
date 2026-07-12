use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_rpc_primitives::Bytes as NativeRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::{espace as service_espace, native as service_native};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Native address: {message}")]
pub(super) struct ResponseMappingError {
    field: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SimulateEspaceTransactionResponse {
    execution: Execution,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Execution {
    chain_id: U64,
    block: SimulatedBlock,
    status: SimulationStatus,
    gas_used: U256,
    gas_limit: U256,
    gas_charged: U256,
    fee: U256,
    burnt_fee: Option<U256>,
    output: RpcBytes,
    failure: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SimulatedBlock {
    number: U64,
    hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SimulationStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExecutionFailure {
    code: ExecutionFailureCode,
    message: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ExecutionFailureCode {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SimulateNativeTransactionResponse {
    execution: NativeExecution,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NativeExecution {
    chain_id: U64,
    state: NativeStateAnchor,
    status: NativeSimulationStatus,
    gas_used: U256,
    gas_limit: U256,
    gas_charged: U256,
    fee: U256,
    burnt_fee: Option<U256>,
    gas_covered_by_sponsor: bool,
    storage_covered_by_sponsor: bool,
    storage_collateralized: Vec<NativeStorageChange>,
    storage_released: Vec<NativeStorageChange>,
    output: NativeRpcBytes,
    failure: Option<NativeExecutionFailure>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NativeStateAnchor {
    epoch_number: U64,
    pivot_hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum NativeSimulationStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NativeStorageChange {
    address: RpcAddress,
    collateral_units: U64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NativeExecutionFailure {
    code: NativeExecutionFailureCode,
    message: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum NativeExecutionFailureCode {
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

impl From<service_espace::SimulateEspaceTransactionOutput> for SimulateEspaceTransactionResponse {
    fn from(output: service_espace::SimulateEspaceTransactionOutput) -> Self {
        Self {
            execution: output.execution.into(),
        }
    }
}

impl From<service_espace::EspaceExecution> for Execution {
    fn from(execution: service_espace::EspaceExecution) -> Self {
        Self {
            chain_id: execution.chain_id.into(),
            block: execution.block.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            gas_charged: execution.gas_charged,
            fee: execution.fee,
            burnt_fee: execution.burnt_fee,
            output: RpcBytes::from(execution.output),
            failure: execution.failure.map(Into::into),
        }
    }
}

impl From<service_espace::SimulatedBlock> for SimulatedBlock {
    fn from(block: service_espace::SimulatedBlock) -> Self {
        Self {
            number: block.number.into(),
            hash: block.hash,
        }
    }
}

impl From<service_espace::EspaceExecutionStatus> for SimulationStatus {
    fn from(status: service_espace::EspaceExecutionStatus) -> Self {
        match status {
            service_espace::EspaceExecutionStatus::Success => Self::Success,
            service_espace::EspaceExecutionStatus::Failed => Self::Failed,
            service_espace::EspaceExecutionStatus::NotExecuted => Self::NotExecuted,
        }
    }
}

impl From<service_espace::EspaceExecutionFailure> for ExecutionFailure {
    fn from(failure: service_espace::EspaceExecutionFailure) -> Self {
        Self {
            code: failure.code.into(),
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<service_espace::EspaceExecutionFailureCode> for ExecutionFailureCode {
    fn from(code: service_espace::EspaceExecutionFailureCode) -> Self {
        match code {
            service_espace::EspaceExecutionFailureCode::ChainIdMismatch => Self::ChainIdMismatch,
            service_espace::EspaceExecutionFailureCode::ZeroGasPrice => Self::ZeroGasPrice,
            service_espace::EspaceExecutionFailureCode::PriorityFeeExceedsMaxFee => {
                Self::PriorityFeeExceedsMaxFee
            }
            service_espace::EspaceExecutionFailureCode::NonceTooLow => Self::NonceTooLow,
            service_espace::EspaceExecutionFailureCode::NonceTooHigh => Self::NonceTooHigh,
            service_espace::EspaceExecutionFailureCode::FeeBelowBaseFee => Self::FeeBelowBaseFee,
            service_espace::EspaceExecutionFailureCode::IntrinsicGasTooLow => {
                Self::IntrinsicGasTooLow
            }
            service_espace::EspaceExecutionFailureCode::SenderWithCode => Self::SenderWithCode,
            service_espace::EspaceExecutionFailureCode::SenderDoesNotExist => {
                Self::SenderDoesNotExist
            }
            service_espace::EspaceExecutionFailureCode::InsufficientFunds => {
                Self::InsufficientFunds
            }
            service_espace::EspaceExecutionFailureCode::Revert => Self::Revert,
            service_espace::EspaceExecutionFailureCode::OutOfGas => Self::OutOfGas,
            service_espace::EspaceExecutionFailureCode::NonceOverflow => Self::NonceOverflow,
            service_espace::EspaceExecutionFailureCode::VmError => Self::VmError,
        }
    }
}

impl SimulateNativeTransactionResponse {
    pub(super) fn try_from_output(
        output: service_native::SimulateNativeTransactionOutput,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        Ok(Self {
            execution: NativeExecution::try_from_service(output.execution, network)?,
        })
    }
}

impl NativeExecution {
    fn try_from_service(
        execution: service_native::SimulationExecution,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        Ok(Self {
            chain_id: execution.chain_id.into(),
            state: execution.state.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            gas_charged: execution.gas_charged,
            fee: execution.fee,
            burnt_fee: execution.burnt_fee,
            gas_covered_by_sponsor: execution.gas_covered_by_sponsor,
            storage_covered_by_sponsor: execution.storage_covered_by_sponsor,
            storage_collateralized: map_native_storage_changes(
                execution.storage_collateralized,
                network,
                "execution.storageCollateralized",
            )?,
            storage_released: map_native_storage_changes(
                execution.storage_released,
                network,
                "execution.storageReleased",
            )?,
            output: NativeRpcBytes::from(execution.output),
            failure: execution.failure.map(Into::into),
        })
    }
}

impl From<service_native::StateAnchor> for NativeStateAnchor {
    fn from(state: service_native::StateAnchor) -> Self {
        Self {
            epoch_number: state.epoch_number.into(),
            pivot_hash: state.pivot_hash,
        }
    }
}

impl From<service_native::ExecutionStatus> for NativeSimulationStatus {
    fn from(status: service_native::ExecutionStatus) -> Self {
        match status {
            service_native::ExecutionStatus::Success => Self::Success,
            service_native::ExecutionStatus::Failed => Self::Failed,
            service_native::ExecutionStatus::NotExecuted => Self::NotExecuted,
        }
    }
}

impl From<service_native::ExecutionFailure> for NativeExecutionFailure {
    fn from(failure: service_native::ExecutionFailure) -> Self {
        Self {
            code: failure.code.into(),
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<service_native::ExecutionFailureCode> for NativeExecutionFailureCode {
    fn from(code: service_native::ExecutionFailureCode) -> Self {
        match code {
            service_native::ExecutionFailureCode::ChainIdMismatch => Self::ChainIdMismatch,
            service_native::ExecutionFailureCode::ZeroGasPrice => Self::ZeroGasPrice,
            service_native::ExecutionFailureCode::PriorityFeeExceedsMaxFee => {
                Self::PriorityFeeExceedsMaxFee
            }
            service_native::ExecutionFailureCode::NonceTooLow => Self::NonceTooLow,
            service_native::ExecutionFailureCode::NonceTooHigh => Self::NonceTooHigh,
            service_native::ExecutionFailureCode::EpochHeightOutOfBound => {
                Self::EpochHeightOutOfBound
            }
            service_native::ExecutionFailureCode::FeeBelowBaseFee => Self::FeeBelowBaseFee,
            service_native::ExecutionFailureCode::IntrinsicGasTooLow => Self::IntrinsicGasTooLow,
            service_native::ExecutionFailureCode::InvalidRecipient => Self::InvalidRecipient,
            service_native::ExecutionFailureCode::SenderWithCode => Self::SenderWithCode,
            service_native::ExecutionFailureCode::SenderDoesNotExist => Self::SenderDoesNotExist,
            service_native::ExecutionFailureCode::InsufficientFunds => Self::InsufficientFunds,
            service_native::ExecutionFailureCode::SponsorBalanceInsufficient => {
                Self::SponsorBalanceInsufficient
            }
            service_native::ExecutionFailureCode::Revert => Self::Revert,
            service_native::ExecutionFailureCode::OutOfGas => Self::OutOfGas,
            service_native::ExecutionFailureCode::StorageBalanceInsufficient => {
                Self::StorageBalanceInsufficient
            }
            service_native::ExecutionFailureCode::StorageLimitExceeded => {
                Self::StorageLimitExceeded
            }
            service_native::ExecutionFailureCode::NonceOverflow => Self::NonceOverflow,
            service_native::ExecutionFailureCode::VmError => Self::VmError,
        }
    }
}

fn map_native_storage_changes(
    changes: Vec<service_native::StorageChange>,
    network: Network,
    field: &str,
) -> Result<Vec<NativeStorageChange>, ResponseMappingError> {
    changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| {
            Ok(NativeStorageChange {
                address: map_native_address(
                    change.address,
                    network,
                    format!("{field}[{index}].address"),
                )?,
                collateral_units: change.collateral_units.into(),
            })
        })
        .collect()
}

fn map_native_address(
    address: Address,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    RpcAddress::try_from_h160(address, network)
        .map_err(|message| ResponseMappingError { field, message })
}

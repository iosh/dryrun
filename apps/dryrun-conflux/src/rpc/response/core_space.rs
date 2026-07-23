use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::core_space as service_core_space;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Core Space address: {message}")]
pub(in crate::rpc) struct ResponseMappingError {
    field: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(in crate::rpc) struct SimulateCoreSpaceTransactionResponse {
    execution: CoreSpaceExecution,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceExecution {
    chain_id: U64,
    state: CoreSpaceStateAnchor,
    status: CoreSpaceExecutionStatus,
    gas_used: U256,
    gas_limit: U256,
    gas_charged: U256,
    fee: U256,
    burnt_fee: Option<U256>,
    gas_covered_by_sponsor: bool,
    storage_covered_by_sponsor: bool,
    storage_collateralized: Vec<CoreSpaceStorageChange>,
    storage_released: Vec<CoreSpaceStorageChange>,
    output: CoreSpaceRpcBytes,
    failure: Option<CoreSpaceExecutionFailure>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceStateAnchor {
    epoch_number: U64,
    pivot_hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CoreSpaceExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceStorageChange {
    address: RpcAddress,
    collateral_units: U64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceExecutionFailure {
    code: CoreSpaceExecutionFailureCode,
    message: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum CoreSpaceExecutionFailureCode {
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

impl SimulateCoreSpaceTransactionResponse {
    pub(in crate::rpc) fn try_from_output(
        output: service_core_space::SimulateCoreSpaceTransactionOutput,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        Ok(Self {
            execution: CoreSpaceExecution::try_from_service(output.execution, network)?,
        })
    }
}

impl CoreSpaceExecution {
    fn try_from_service(
        execution: service_core_space::CoreSpaceExecution,
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
            storage_collateralized: map_core_space_storage_changes(
                execution.storage_collateralized,
                network,
                "execution.storageCollateralized",
            )?,
            storage_released: map_core_space_storage_changes(
                execution.storage_released,
                network,
                "execution.storageReleased",
            )?,
            output: CoreSpaceRpcBytes::from(execution.output),
            failure: execution.failure.map(Into::into),
        })
    }
}

impl From<service_core_space::CoreSpaceStateAnchor> for CoreSpaceStateAnchor {
    fn from(state: service_core_space::CoreSpaceStateAnchor) -> Self {
        Self {
            epoch_number: state.epoch_number.into(),
            pivot_hash: state.pivot_hash,
        }
    }
}

impl From<service_core_space::CoreSpaceExecutionStatus> for CoreSpaceExecutionStatus {
    fn from(status: service_core_space::CoreSpaceExecutionStatus) -> Self {
        match status {
            service_core_space::CoreSpaceExecutionStatus::Success => Self::Success,
            service_core_space::CoreSpaceExecutionStatus::Failed => Self::Failed,
            service_core_space::CoreSpaceExecutionStatus::NotExecuted => Self::NotExecuted,
        }
    }
}

impl From<service_core_space::CoreSpaceExecutionFailure> for CoreSpaceExecutionFailure {
    fn from(failure: service_core_space::CoreSpaceExecutionFailure) -> Self {
        Self {
            code: failure.code.into(),
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<service_core_space::CoreSpaceExecutionFailureCode> for CoreSpaceExecutionFailureCode {
    fn from(code: service_core_space::CoreSpaceExecutionFailureCode) -> Self {
        match code {
            service_core_space::CoreSpaceExecutionFailureCode::ChainIdMismatch => {
                Self::ChainIdMismatch
            }
            service_core_space::CoreSpaceExecutionFailureCode::ZeroGasPrice => Self::ZeroGasPrice,
            service_core_space::CoreSpaceExecutionFailureCode::PriorityFeeExceedsMaxFee => {
                Self::PriorityFeeExceedsMaxFee
            }
            service_core_space::CoreSpaceExecutionFailureCode::NonceTooLow => Self::NonceTooLow,
            service_core_space::CoreSpaceExecutionFailureCode::NonceTooHigh => Self::NonceTooHigh,
            service_core_space::CoreSpaceExecutionFailureCode::EpochHeightOutOfBound => {
                Self::EpochHeightOutOfBound
            }
            service_core_space::CoreSpaceExecutionFailureCode::FeeBelowBaseFee => {
                Self::FeeBelowBaseFee
            }
            service_core_space::CoreSpaceExecutionFailureCode::IntrinsicGasTooLow => {
                Self::IntrinsicGasTooLow
            }
            service_core_space::CoreSpaceExecutionFailureCode::InvalidRecipient => {
                Self::InvalidRecipient
            }
            service_core_space::CoreSpaceExecutionFailureCode::SenderWithCode => {
                Self::SenderWithCode
            }
            service_core_space::CoreSpaceExecutionFailureCode::SenderDoesNotExist => {
                Self::SenderDoesNotExist
            }
            service_core_space::CoreSpaceExecutionFailureCode::InsufficientFunds => {
                Self::InsufficientFunds
            }
            service_core_space::CoreSpaceExecutionFailureCode::SponsorBalanceInsufficient => {
                Self::SponsorBalanceInsufficient
            }
            service_core_space::CoreSpaceExecutionFailureCode::Revert => Self::Revert,
            service_core_space::CoreSpaceExecutionFailureCode::OutOfGas => Self::OutOfGas,
            service_core_space::CoreSpaceExecutionFailureCode::StorageBalanceInsufficient => {
                Self::StorageBalanceInsufficient
            }
            service_core_space::CoreSpaceExecutionFailureCode::StorageLimitExceeded => {
                Self::StorageLimitExceeded
            }
            service_core_space::CoreSpaceExecutionFailureCode::NonceOverflow => Self::NonceOverflow,
            service_core_space::CoreSpaceExecutionFailureCode::VmError => Self::VmError,
        }
    }
}

fn map_core_space_storage_changes(
    changes: Vec<service_core_space::CoreSpaceStorageChange>,
    network: Network,
    field: &str,
) -> Result<Vec<CoreSpaceStorageChange>, ResponseMappingError> {
    changes
        .into_iter()
        .enumerate()
        .map(|(index, change)| {
            Ok(CoreSpaceStorageChange {
                address: map_core_space_address(
                    change.address,
                    network,
                    format!("{field}[{index}].address"),
                )?,
                collateral_units: change.collateral_units.into(),
            })
        })
        .collect()
}

fn map_core_space_address(
    address: Address,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    RpcAddress::try_from_h160(address, network)
        .map_err(|message| ResponseMappingError { field, message })
}

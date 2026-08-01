use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::core_space as service_core_space;
use serde::Serialize;

use super::{core_space_change, u256_to_wire};

#[derive(Debug, thiserror::Error)]
#[error("failed to encode `{field}` as a Core Space address: {message}")]
pub(crate) struct ResponseMappingError {
    field: String,
    message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateCoreSpaceTransactionResponse {
    execution: CoreSpaceExecution,
    changes: Vec<core_space_change::Change>,
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
    pub(crate) fn try_from_output(
        simulation: service_core_space::SimulateCoreSpaceTransactionOutput,
        network: Network,
    ) -> Result<Self, ResponseMappingError> {
        let (execution, changes) = simulation.into_parts();
        Ok(Self {
            execution: CoreSpaceExecution::from_service(execution),
            changes: core_space_change::try_map_changes(changes, network)?,
        })
    }
}

impl CoreSpaceExecution {
    fn from_service(execution: service_core_space::CoreSpaceExecution) -> Self {
        let service_core_space::CoreSpaceExecution {
            chain_id,
            context: state,
            gas_limit,
            outcome,
        } = execution;
        let (
            status,
            gas_used,
            gas_charged,
            fee,
            burnt_fee,
            gas_covered_by_sponsor,
            storage_covered_by_sponsor,
            output,
            failure,
        ) = match outcome {
            service_core_space::CoreSpaceExecutionOutcome::Success(details) => {
                let common = details.common;
                (
                    CoreSpaceExecutionStatus::Success,
                    common.gas_used.into(),
                    common.gas_charged.into(),
                    u256_to_wire(common.fee),
                    common.burnt_fee.map(u256_to_wire),
                    details.gas_covered_by_sponsor,
                    details.storage_covered_by_sponsor,
                    CoreSpaceRpcBytes::from(common.output.to_vec()),
                    None,
                )
            }
            service_core_space::CoreSpaceExecutionOutcome::Failed { details, failure } => {
                let common = details.common;
                (
                    CoreSpaceExecutionStatus::Failed,
                    common.gas_used.into(),
                    common.gas_charged.into(),
                    u256_to_wire(common.fee),
                    common.burnt_fee.map(u256_to_wire),
                    details.gas_covered_by_sponsor,
                    details.storage_covered_by_sponsor,
                    CoreSpaceRpcBytes::from(common.output.to_vec()),
                    Some(failure.into()),
                )
            }
            service_core_space::CoreSpaceExecutionOutcome::NotExecuted(failure) => (
                CoreSpaceExecutionStatus::NotExecuted,
                U256::zero(),
                U256::zero(),
                U256::zero(),
                Some(U256::zero()),
                false,
                false,
                CoreSpaceRpcBytes::default(),
                Some(failure.into()),
            ),
        };

        Self {
            chain_id: chain_id.into(),
            state: state.into(),
            status,
            gas_used,
            gas_limit: gas_limit.into(),
            gas_charged,
            fee,
            burnt_fee,
            gas_covered_by_sponsor,
            storage_covered_by_sponsor,
            output,
            failure,
        }
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

pub(super) fn map_core_space_address(
    address: Address,
    network: Network,
    field: String,
) -> Result<RpcAddress, ResponseMappingError> {
    RpcAddress::try_from_h160(address, network)
        .map_err(|message| ResponseMappingError { field, message })
}

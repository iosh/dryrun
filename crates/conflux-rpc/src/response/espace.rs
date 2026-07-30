use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_service::espace as service_espace;
use serde::Serialize;

use super::{b256_to_wire, u256_to_wire};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateEspaceTransactionResponse {
    execution: Execution,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Execution {
    chain_id: U64,
    block: SimulatedBlock,
    status: EspaceExecutionStatus,
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
enum EspaceExecutionStatus {
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

impl From<service_espace::SimulateEspaceTransactionOutput> for SimulateEspaceTransactionResponse {
    fn from(execution: service_espace::SimulateEspaceTransactionOutput) -> Self {
        Self {
            execution: execution.into(),
        }
    }
}

impl From<service_espace::EspaceExecution> for Execution {
    fn from(execution: service_espace::EspaceExecution) -> Self {
        let service_espace::EspaceExecution {
            chain_id,
            context: block,
            gas_limit,
            outcome,
        } = execution;
        let (status, gas_used, gas_charged, fee, burnt_fee, output, failure) = match outcome {
            service_espace::EspaceExecutionOutcome::Success(details) => (
                EspaceExecutionStatus::Success,
                details.gas_used.into(),
                details.gas_charged.into(),
                u256_to_wire(details.fee),
                details.burnt_fee.map(u256_to_wire),
                RpcBytes::from(details.output.to_vec()),
                None,
            ),
            service_espace::EspaceExecutionOutcome::Failed { details, failure } => (
                EspaceExecutionStatus::Failed,
                details.gas_used.into(),
                details.gas_charged.into(),
                u256_to_wire(details.fee),
                details.burnt_fee.map(u256_to_wire),
                RpcBytes::from(details.output.to_vec()),
                Some(failure.into()),
            ),
            service_espace::EspaceExecutionOutcome::NotExecuted(failure) => (
                EspaceExecutionStatus::NotExecuted,
                U256::zero(),
                U256::zero(),
                U256::zero(),
                Some(U256::zero()),
                RpcBytes::default(),
                Some(failure.into()),
            ),
        };

        Self {
            chain_id: chain_id.into(),
            block: block.into(),
            status,
            gas_used,
            gas_limit: gas_limit.into(),
            gas_charged,
            fee,
            burnt_fee,
            output,
            failure,
        }
    }
}

impl From<service_espace::SimulatedBlock> for SimulatedBlock {
    fn from(block: service_espace::SimulatedBlock) -> Self {
        Self {
            number: block.number.into(),
            hash: b256_to_wire(block.hash),
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

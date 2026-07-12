use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_service::espace as service_espace;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(in crate::rpc) struct SimulateEspaceTransactionResponse {
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

impl From<service_espace::EspaceExecutionStatus> for EspaceExecutionStatus {
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


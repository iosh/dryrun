use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_simulation::espace::{
    EspaceBlockContext as SimulationBlockContext, EspaceExecutionFailure, EspaceExecutionOutcome,
    EspaceSimulation, EspaceSuccessOutput, EspaceTransactionRejection,
};
use serde::Serialize;

use super::{b256_to_wire, change::Change, u256_to_wire};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SimulateEspaceTransactionResponse {
    execution: Execution,
    changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Execution {
    chain_id: U64,
    block: EspaceBlockContext,
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
struct EspaceBlockContext {
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
    TransactionTypeNotActivated,
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

impl From<EspaceSimulation> for SimulateEspaceTransactionResponse {
    fn from(simulation: EspaceSimulation) -> Self {
        let EspaceSimulation {
            context,
            transaction,
            execution,
            changes,
        } = simulation;
        let common = transaction.common();
        let gas_limit = common.gas_limit;
        let chain_id = common.chain_id;

        let (status, result, output, failure) = match execution {
            EspaceExecutionOutcome::Success { result, output, .. } => {
                let output = match output {
                    EspaceSuccessOutput::Call { return_data } => return_data,
                    EspaceSuccessOutput::Create { runtime_code, .. } => runtime_code,
                };
                (EspaceExecutionStatus::Success, Some(result), output, None)
            }
            EspaceExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            } => (
                EspaceExecutionStatus::Failed,
                Some(result),
                revert_data,
                Some(ExecutionFailure {
                    code: ExecutionFailureCode::Revert,
                    message: "execution reverted".to_owned(),
                    reason: reason.map(|reason| reason.to_string()),
                }),
            ),
            EspaceExecutionOutcome::Failed { result, failure } => (
                EspaceExecutionStatus::Failed,
                Some(result),
                Default::default(),
                Some(failure.into()),
            ),
            EspaceExecutionOutcome::NotExecuted(rejection) => (
                EspaceExecutionStatus::NotExecuted,
                None,
                Default::default(),
                Some(rejection.into()),
            ),
        };
        let (gas_used, gas_charged, fee, burnt_fee) = result.map_or(
            (
                0,
                0,
                alloy_primitives::U256::ZERO,
                Some(alloy_primitives::U256::ZERO),
            ),
            |result| {
                (
                    result.gas().gas_used(),
                    result.gas().gas_charged(),
                    result.fee().charged_amount(),
                    result.fee().burnt_amount(),
                )
            },
        );

        Self {
            execution: Execution {
                chain_id: chain_id.into(),
                block: context.into(),
                status,
                gas_used: gas_used.into(),
                gas_limit: gas_limit.into(),
                gas_charged: gas_charged.into(),
                fee: u256_to_wire(fee),
                burnt_fee: burnt_fee.map(u256_to_wire),
                output: RpcBytes::from(output.to_vec()),
                failure,
            },
            changes: changes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SimulationBlockContext> for EspaceBlockContext {
    fn from(block: SimulationBlockContext) -> Self {
        Self {
            number: block.number.into(),
            hash: b256_to_wire(block.hash),
        }
    }
}

impl From<EspaceExecutionFailure> for ExecutionFailure {
    fn from(failure: EspaceExecutionFailure) -> Self {
        Self {
            code: execution_failure_code(&failure),
            message: failure.to_string(),
            reason: None,
        }
    }
}

impl From<EspaceTransactionRejection> for ExecutionFailure {
    fn from(rejection: EspaceTransactionRejection) -> Self {
        Self {
            code: rejection_failure_code(&rejection),
            message: rejection.to_string(),
            reason: None,
        }
    }
}

fn execution_failure_code(failure: &EspaceExecutionFailure) -> ExecutionFailureCode {
    match failure {
        EspaceExecutionFailure::InsufficientFunds { .. } => ExecutionFailureCode::InsufficientFunds,
        EspaceExecutionFailure::OutOfGas => ExecutionFailureCode::OutOfGas,
        EspaceExecutionFailure::NonceOverflow { .. } => ExecutionFailureCode::NonceOverflow,
        _ => ExecutionFailureCode::VmError,
    }
}

fn rejection_failure_code(rejection: &EspaceTransactionRejection) -> ExecutionFailureCode {
    match rejection {
        EspaceTransactionRejection::InvalidChainId { .. } => ExecutionFailureCode::ChainIdMismatch,
        EspaceTransactionRejection::ZeroGasPrice => ExecutionFailureCode::ZeroGasPrice,
        EspaceTransactionRejection::PriorityFeeGreaterThanMaxFee { .. } => {
            ExecutionFailureCode::PriorityFeeExceedsMaxFee
        }
        EspaceTransactionRejection::CalldataGasRequirement { .. }
        | EspaceTransactionRejection::IntrinsicGasExceedsGasLimit { .. } => {
            ExecutionFailureCode::IntrinsicGasTooLow
        }
        EspaceTransactionRejection::NonceTooLow { .. } => ExecutionFailureCode::NonceTooLow,
        EspaceTransactionRejection::NonceTooHigh { .. } => ExecutionFailureCode::NonceTooHigh,
        EspaceTransactionRejection::SenderHasCode { .. } => ExecutionFailureCode::SenderWithCode,
        EspaceTransactionRejection::SenderDoesNotExist => ExecutionFailureCode::SenderDoesNotExist,
        EspaceTransactionRejection::GasPriceBelowBaseFee { .. } => {
            ExecutionFailureCode::FeeBelowBaseFee
        }
        EspaceTransactionRejection::InsufficientFunds { .. } => {
            ExecutionFailureCode::InsufficientFunds
        }
        EspaceTransactionRejection::LegacyTransactionNotActivated
        | EspaceTransactionRejection::Eip2930NotActivated
        | EspaceTransactionRejection::Eip1559NotActivated
        | EspaceTransactionRejection::Eip7702NotActivated => {
            ExecutionFailureCode::TransactionTypeNotActivated
        }
        EspaceTransactionRejection::CreateInitCodeSizeLimit { .. } => ExecutionFailureCode::VmError,
        _ => ExecutionFailureCode::VmError,
    }
}

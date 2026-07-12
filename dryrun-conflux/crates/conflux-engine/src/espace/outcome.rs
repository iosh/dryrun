use cfx_bytes::Bytes;
use cfx_executor::executive::{ExecutionError, ExecutionOutcome, ToRepackError, TxDropError};
use cfx_types::U256;
use cfx_vm_types as vm;

use super::{
    EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode, EspaceExecutionStatus,
    SimulatedBlock,
};
use crate::ConfluxEngineError;

pub(crate) fn build_espace_not_executed(
    chain_id: u32,
    block: SimulatedBlock,
    gas_limit: U256,
    failure: EspaceExecutionFailure,
) -> EspaceExecution {
    EspaceExecution {
        chain_id: u64::from(chain_id),
        block,
        status: EspaceExecutionStatus::NotExecuted,
        gas_used: U256::zero(),
        gas_limit,
        gas_charged: U256::zero(),
        fee: U256::zero(),
        burnt_fee: Some(U256::zero()),
        output: Bytes::new(),
        failure: Some(failure),
    }
}

pub(crate) fn build_espace_execution(
    chain_id: u32,
    block: SimulatedBlock,
    gas_limit: U256,
    outcome: ExecutionOutcome,
) -> Result<EspaceExecution, ConfluxEngineError> {
    let failure = build_failure(&outcome)?;
    let status = if failure.is_some() {
        EspaceExecutionStatus::Failed
    } else {
        EspaceExecutionStatus::Success
    };

    let executed = outcome.try_into_executed();

    Ok(EspaceExecution {
        chain_id: u64::from(chain_id),
        block,
        status,
        gas_used: executed
            .as_ref()
            .map(|executed| executed.gas_used)
            .unwrap_or_else(U256::zero),
        gas_limit,
        gas_charged: executed
            .as_ref()
            .map(|executed| executed.gas_charged)
            .unwrap_or_else(U256::zero),
        fee: executed
            .as_ref()
            .map(|executed| executed.fee)
            .unwrap_or_else(U256::zero),
        burnt_fee: executed.as_ref().and_then(|executed| executed.burnt_fee),
        output: executed.map(|executed| executed.output).unwrap_or_default(),
        failure,
    })
}
fn build_failure(
    outcome: &ExecutionOutcome,
) -> Result<Option<EspaceExecutionFailure>, ConfluxEngineError> {
    match outcome {
        ExecutionOutcome::Finished(_) => Ok(None),
        ExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => {
            build_execution_error_failure(error, executed.output.as_ref()).map(Some)
        }
        ExecutionOutcome::NotExecutedDrop(error) => build_espace_drop_failure(error).map(Some),
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            build_espace_repack_failure(error).map(Some)
        }
    }
}

fn espace_failure(
    code: EspaceExecutionFailureCode,
    message: impl Into<String>,
) -> EspaceExecutionFailure {
    EspaceExecutionFailure {
        code,
        message: message.into(),
        reason: None,
    }
}

fn build_espace_drop_failure(
    error: &TxDropError,
) -> Result<EspaceExecutionFailure, ConfluxEngineError> {
    match error {
        TxDropError::OldNonce(expected, got) => Ok(espace_failure(
            EspaceExecutionFailureCode::NonceTooLow,
            format!("transaction nonce {got} is lower than state nonce {expected}"),
        )),
        TxDropError::NotEnoughGasLimit { expected, got } => Ok(espace_failure(
            EspaceExecutionFailureCode::IntrinsicGasTooLow,
            format!("transaction gas limit {got} is lower than intrinsic gas {expected}"),
        )),
        TxDropError::SenderWithCode(address) => Ok(espace_failure(
            EspaceExecutionFailureCode::SenderWithCode,
            format!("transaction sender has contract code: {address:?}"),
        )),
        TxDropError::InvalidRecipientAddress(address) => {
            Err(ConfluxEngineError::ExecutionInternal {
                message: format!(
                    "eSpace execution returned Native-only invalid recipient: \
                       {address:?}"
                ),
            })
        }
    }
}

fn build_espace_repack_failure(
    error: &ToRepackError,
) -> Result<EspaceExecutionFailure, ConfluxEngineError> {
    match error {
        ToRepackError::InvalidNonce { expected, got } => Ok(espace_failure(
            EspaceExecutionFailureCode::NonceTooHigh,
            format!("transaction nonce {got} is higher than state nonce {expected}"),
        )),
        ToRepackError::SenderDoesNotExist => Ok(espace_failure(
            EspaceExecutionFailureCode::SenderDoesNotExist,
            "transaction sender does not exist",
        )),
        ToRepackError::NotEnoughBaseFee { expected, got } => Ok(espace_failure(
            EspaceExecutionFailureCode::FeeBelowBaseFee,
            format!("transaction gas price {got} is lower than required base fee {expected}"),
        )),
        ToRepackError::NotEnoughBalance { expected, got } => Ok(espace_failure(
            EspaceExecutionFailureCode::InsufficientFunds,
            format!("sender balance {got} is lower than required cost {expected}"),
        )),
        ToRepackError::EpochHeightOutOfBound { .. } => Err(ConfluxEngineError::ExecutionInternal {
            message: format!(
                "eSpace execution returned Native-only epoch validation error: \
                       {error:?}"
            ),
        }),
        ToRepackError::NotEnoughCashFromSponsor { .. } => {
            Err(ConfluxEngineError::ExecutionInternal {
                message: format!("eSpace execution returned Native-only sponsor error: {error:?}"),
            })
        }
    }
}

fn build_execution_error_failure(
    error: &ExecutionError,
    output: &[u8],
) -> Result<EspaceExecutionFailure, ConfluxEngineError> {
    match error {
        ExecutionError::NotEnoughCash {
            required,
            got,
            actual_gas_cost,
            max_storage_limit_cost,
        } => Ok(espace_failure(
            EspaceExecutionFailureCode::InsufficientFunds,
            format!(
                "sender balance {got} is lower than required cost {required}; \
                 actual gas cost is {actual_gas_cost}, maximum storage cost is \
                 {max_storage_limit_cost}"
            ),
        )),
        ExecutionError::NonceOverflow(address) => Ok(espace_failure(
            EspaceExecutionFailureCode::NonceOverflow,
            format!("nonce overflow for address: {address:?}"),
        )),
        ExecutionError::VmError(error) => build_espace_vm_failure(error, output),
    }
}

fn build_espace_vm_failure(
    error: &vm::Error,
    output: &[u8],
) -> Result<EspaceExecutionFailure, ConfluxEngineError> {
    match error {
        vm::Error::Reverted => Ok(EspaceExecutionFailure {
            code: EspaceExecutionFailureCode::Revert,
            message: "execution reverted".to_string(),
            reason: revert_reason(output),
        }),
        vm::Error::OutOfGas => Ok(espace_failure(
            EspaceExecutionFailureCode::OutOfGas,
            "execution ran out of gas",
        )),
        vm::Error::NonceOverflow(address) => Ok(espace_failure(
            EspaceExecutionFailureCode::NonceOverflow,
            format!("nonce overflow for address: {address:?}"),
        )),
        vm::Error::StateDbError(error) => Err(ConfluxEngineError::StateAccess {
            message: format!("{error:?}"),
        }),
        vm::Error::NotEnoughBalanceForStorage { .. } | vm::Error::ExceedStorageLimit => {
            Err(ConfluxEngineError::ExecutionInternal {
                message: format!("eSpace execution returned Native-only storage error: {error}"),
            })
        }
        vm::Error::BadJumpDestination { .. }
        | vm::Error::BadInstruction { .. }
        | vm::Error::StackUnderflow { .. }
        | vm::Error::OutOfStack { .. }
        | vm::Error::SubStackUnderflow { .. }
        | vm::Error::OutOfSubStack { .. }
        | vm::Error::InvalidSubEntry
        | vm::Error::BuiltIn(_)
        | vm::Error::InternalContract(_)
        | vm::Error::MutableCallInStaticContext
        | vm::Error::CreateInitCodeSizeLimit
        | vm::Error::Wasm(_)
        | vm::Error::OutOfBounds
        | vm::Error::InvalidAddress(_)
        | vm::Error::ConflictAddress(_)
        | vm::Error::CreateContractStartingWithEF => Ok(espace_failure(
            EspaceExecutionFailureCode::VmError,
            format!("virtual machine execution failed: {error}"),
        )),
    }
}

// TODO
fn revert_reason(_output: &[u8]) -> Option<String> {
    None
}


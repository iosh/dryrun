use cfx_executor::executive::{ExecutionError, ToRepackError, TxDropError};
use cfx_vm_types as vm;

use super::{
    EspaceExecutedDetails, EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceExecutionOutcome, SimulatedBlock,
};
use crate::{ConfluxSimulationError, execution::TransactionExecutionOutcome};

pub(crate) fn build_espace_not_executed(
    chain_id: u32,
    block: SimulatedBlock,
    gas_limit: u64,
    failure: EspaceExecutionFailure,
) -> EspaceExecution {
    EspaceExecution {
        chain_id: u64::from(chain_id),
        context: block,
        gas_limit,
        outcome: EspaceExecutionOutcome::NotExecuted(failure),
    }
}

pub(crate) fn build_espace_execution(
    chain_id: u32,
    block: SimulatedBlock,
    gas_limit: u64,
    outcome: TransactionExecutionOutcome,
) -> Result<EspaceExecution, ConfluxSimulationError> {
    let outcome = match outcome {
        TransactionExecutionOutcome::Success(details) => {
            EspaceExecutionOutcome::Success(into_espace_details(details))
        }
        TransactionExecutionOutcome::Failed { error, details } => {
            let failure = build_execution_error_failure(&error, details.common.output.as_ref())?;
            EspaceExecutionOutcome::Failed {
                details: into_espace_details(details),
                failure,
            }
        }
        TransactionExecutionOutcome::NotExecutedDrop(error) => {
            EspaceExecutionOutcome::NotExecuted(build_espace_drop_failure(&error)?)
        }
        TransactionExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            EspaceExecutionOutcome::NotExecuted(build_espace_repack_failure(&error)?)
        }
    };

    Ok(EspaceExecution {
        chain_id: u64::from(chain_id),
        context: block,
        gas_limit,
        outcome,
    })
}

fn into_espace_details(details: crate::execution::ConfluxExecutionOutput) -> EspaceExecutedDetails {
    let crate::execution::ConfluxExecutionOutput { common, .. } = details;
    EspaceExecutedDetails {
        gas_used: common.gas_used,
        gas_charged: common.gas_charged,
        fee: common.fee,
        burnt_fee: common.burnt_fee,
        output: common.output,
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
) -> Result<EspaceExecutionFailure, ConfluxSimulationError> {
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
            Err(ConfluxSimulationError::ExecutionInternal {
                message: format!(
                    "eSpace execution returned Core Space-specific invalid recipient: \
                       {address:?}"
                ),
            })
        }
    }
}

fn build_espace_repack_failure(
    error: &ToRepackError,
) -> Result<EspaceExecutionFailure, ConfluxSimulationError> {
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
        ToRepackError::EpochHeightOutOfBound { .. } => {
            Err(ConfluxSimulationError::ExecutionInternal {
                message: format!(
                    "eSpace execution returned Core Space-specific epoch validation error: \
                       {error:?}"
                ),
            })
        }
        ToRepackError::NotEnoughCashFromSponsor { .. } => {
            Err(ConfluxSimulationError::ExecutionInternal {
                message: format!(
                    "eSpace execution returned Core Space-specific sponsor error: {error:?}"
                ),
            })
        }
    }
}

fn build_execution_error_failure(
    error: &ExecutionError,
    output: &[u8],
) -> Result<EspaceExecutionFailure, ConfluxSimulationError> {
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
) -> Result<EspaceExecutionFailure, ConfluxSimulationError> {
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
        vm::Error::StateDbError(error) => Err(ConfluxSimulationError::StateAccess {
            message: format!("{error:?}"),
        }),
        vm::Error::NotEnoughBalanceForStorage { .. } | vm::Error::ExceedStorageLimit => {
            Err(ConfluxSimulationError::ExecutionInternal {
                message: format!(
                    "eSpace execution returned Core Space-specific storage error: {error}"
                ),
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

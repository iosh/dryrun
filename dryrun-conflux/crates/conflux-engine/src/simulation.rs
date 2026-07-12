use cfx_bytes::Bytes;
use cfx_executor::executive::{ExecutionError, ExecutionOutcome, ToRepackError, TxDropError};
use cfx_types::U256;
use cfx_vm_types as vm;
use primitives::receipt::StorageChange;

use crate::{
    ConfluxEngineError,
    espace::{
        EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode, EspaceExecutionStatus,
        SimulatedBlock,
    },
    native::{
        NativeExecution, NativeExecutionFailure, NativeExecutionFailureCode,
        NativeExecutionStatus, NativeStateAnchor, NativeStorageChange,
    },
};

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
pub(crate) fn build_native_execution(
    chain_id: u32,
    state: NativeStateAnchor,
    gas_limit: U256,
    outcome: ExecutionOutcome,
) -> NativeExecution {
    let status = match &outcome {
        ExecutionOutcome::Finished(_) => NativeExecutionStatus::Success,
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _) => NativeExecutionStatus::Failed,
        ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => NativeExecutionStatus::NotExecuted,
    };

    let failure = build_native_failure(&outcome);
    let executed = outcome.try_into_executed();

    let storage_collateralized = if status == NativeExecutionStatus::Success {
        executed
            .as_ref()
            .map(|executed| map_native_storage_changes(&executed.storage_collateralized))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let storage_released = if status == NativeExecutionStatus::Success {
        executed
            .as_ref()
            .map(|executed| map_native_storage_changes(&executed.storage_released))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let burnt_fee = if status == NativeExecutionStatus::NotExecuted {
        Some(U256::zero())
    } else {
        executed.as_ref().and_then(|executed| executed.burnt_fee)
    };

    NativeExecution {
        chain_id: u64::from(chain_id),
        state,
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
        burnt_fee,
        gas_covered_by_sponsor: executed
            .as_ref()
            .map(|executed| executed.gas_sponsor_paid)
            .unwrap_or(false),
        storage_covered_by_sponsor: executed
            .as_ref()
            .map(|executed| executed.storage_sponsor_paid)
            .unwrap_or(false),
        storage_collateralized,
        storage_released,
        output: executed.map(|executed| executed.output).unwrap_or_default(),
        failure,
    }
}

pub(crate) fn build_native_not_executed(
    chain_id: u32,
    state: NativeStateAnchor,
    gas_limit: U256,
    failure: NativeExecutionFailure,
) -> NativeExecution {
    NativeExecution {
        chain_id: u64::from(chain_id),
        state,
        status: NativeExecutionStatus::NotExecuted,
        gas_used: U256::zero(),
        gas_limit,
        gas_charged: U256::zero(),
        fee: U256::zero(),
        burnt_fee: Some(U256::zero()),
        gas_covered_by_sponsor: false,
        storage_covered_by_sponsor: false,
        storage_collateralized: Vec::new(),
        storage_released: Vec::new(),
        output: Bytes::new(),
        failure: Some(failure),
    }
}

fn map_native_storage_changes(changes: &[StorageChange]) -> Vec<NativeStorageChange> {
    changes
        .iter()
        .map(|change| NativeStorageChange {
            address: change.address,
            collateral_units: change.collaterals.as_u64(),
        })
        .collect()
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

fn build_native_failure(outcome: &ExecutionOutcome) -> Option<NativeExecutionFailure> {
    match outcome {
        ExecutionOutcome::Finished(_) => None,
        ExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => Some(
            build_native_execution_error_failure(error, executed.output.as_ref()),
        ),
        ExecutionOutcome::NotExecutedDrop(error) => Some(build_native_drop_failure(error)),
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            Some(build_native_repack_failure(error))
        }
    }
}

fn build_native_drop_failure(error: &TxDropError) -> NativeExecutionFailure {
    match error {
        TxDropError::OldNonce(expected, got) => native_failure(
            NativeExecutionFailureCode::NonceTooLow,
            format!("transaction nonce {got} is lower than state nonce {expected}"),
        ),
        TxDropError::InvalidRecipientAddress(address) => native_failure(
            NativeExecutionFailureCode::InvalidRecipient,
            format!("invalid Native recipient address: {address:?}"),
        ),
        TxDropError::NotEnoughGasLimit { expected, got } => native_failure(
            NativeExecutionFailureCode::IntrinsicGasTooLow,
            format!("transaction gas limit {got} is lower than intrinsic gas {expected}"),
        ),
        TxDropError::SenderWithCode(address) => native_failure(
            NativeExecutionFailureCode::SenderWithCode,
            format!("transaction sender has contract code: {address:?}"),
        ),
    }
}

fn build_native_repack_failure(error: &ToRepackError) -> NativeExecutionFailure {
    match error {
        ToRepackError::InvalidNonce { expected, got } => native_failure(
            NativeExecutionFailureCode::NonceTooHigh,
            format!("transaction nonce {got} is higher than state nonce {expected}"),
        ),
        ToRepackError::EpochHeightOutOfBound {
            block_height,
            set,
            transaction_epoch_bound,
        } => native_failure(
            NativeExecutionFailureCode::EpochHeightOutOfBound,
            format!(
                "transaction epoch height {set} is outside execution epoch \
                   {block_height} bound {transaction_epoch_bound}"
            ),
        ),
        ToRepackError::NotEnoughCashFromSponsor {
            required_gas_cost,
            gas_sponsor_balance,
            required_storage_cost,
            storage_sponsor_balance,
        } => native_failure(
            NativeExecutionFailureCode::SponsorBalanceInsufficient,
            format!(
                "sponsor balance is insufficient: required gas \
                   {required_gas_cost}, available gas {gas_sponsor_balance}, \
                   required storage {required_storage_cost}, available storage \
                   {storage_sponsor_balance}"
            ),
        ),
        ToRepackError::SenderDoesNotExist => native_failure(
            NativeExecutionFailureCode::SenderDoesNotExist,
            "transaction sender does not exist",
        ),
        ToRepackError::NotEnoughBaseFee { expected, got } => native_failure(
            NativeExecutionFailureCode::FeeBelowBaseFee,
            format!(
                "transaction gas price {got} is lower than required base fee \
                   {expected}"
            ),
        ),
        ToRepackError::NotEnoughBalance { expected, got } => native_failure(
            NativeExecutionFailureCode::InsufficientFunds,
            format!("sender balance {got} is lower than required cost {expected}"),
        ),
    }
}

fn native_failure(
    code: NativeExecutionFailureCode,
    message: impl Into<String>,
) -> NativeExecutionFailure {
    NativeExecutionFailure {
        code,
        message: message.into(),
        reason: None,
    }
}

fn build_native_execution_error_failure(
    error: &ExecutionError,
    output: &[u8],
) -> NativeExecutionFailure {
    match error {
        ExecutionError::NotEnoughCash {
            required,
            got,
            actual_gas_cost,
            max_storage_limit_cost,
        } => native_failure(
            NativeExecutionFailureCode::InsufficientFunds,
            format!(
                "sender balance {got} is lower than required cost {required}; \
                   actual gas cost is {actual_gas_cost}, maximum storage cost is \
                   {max_storage_limit_cost}"
            ),
        ),
        ExecutionError::NonceOverflow(address) => native_failure(
            NativeExecutionFailureCode::NonceOverflow,
            format!("nonce overflow for address: {address:?}"),
        ),
        ExecutionError::VmError(error) => build_native_vm_failure(error, output),
    }
}

fn build_native_vm_failure(error: &vm::Error, output: &[u8]) -> NativeExecutionFailure {
    match error {
        vm::Error::Reverted => NativeExecutionFailure {
            code: NativeExecutionFailureCode::Revert,
            message: "execution reverted".to_string(),
            reason: revert_reason(output),
        },
        vm::Error::OutOfGas => native_failure(
            NativeExecutionFailureCode::OutOfGas,
            "execution ran out of gas",
        ),
        vm::Error::NotEnoughBalanceForStorage { required, got } => native_failure(
            NativeExecutionFailureCode::StorageBalanceInsufficient,
            format!(
                "storage collateral balance {got} is lower than required \
                       amount {required}"
            ),
        ),
        vm::Error::ExceedStorageLimit => native_failure(
            NativeExecutionFailureCode::StorageLimitExceeded,
            "execution exceeded the transaction storage limit",
        ),
        vm::Error::NonceOverflow(address) => native_failure(
            NativeExecutionFailureCode::NonceOverflow,
            format!("nonce overflow for address: {address:?}"),
        ),
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
        | vm::Error::StateDbError(_)
        | vm::Error::Wasm(_)
        | vm::Error::OutOfBounds
        | vm::Error::InvalidAddress(_)
        | vm::Error::ConflictAddress(_)
        | vm::Error::CreateContractStartingWithEF => native_failure(
            NativeExecutionFailureCode::VmError,
            format!("virtual machine execution failed: {error}"),
        ),
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

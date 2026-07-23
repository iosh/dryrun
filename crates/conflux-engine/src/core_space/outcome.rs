use cfx_bytes::Bytes;
use cfx_executor::executive::{ExecutionError, ExecutionOutcome, ToRepackError, TxDropError};
use cfx_types::U256;
use cfx_vm_types as vm;
use primitives::receipt::StorageChange;

use super::{
    CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
    CoreSpaceExecutionStatus, CoreSpaceStateAnchor, CoreSpaceStorageChange,
};

pub(crate) fn build_core_space_execution(
    chain_id: u32,
    state: CoreSpaceStateAnchor,
    gas_limit: U256,
    outcome: ExecutionOutcome,
) -> CoreSpaceExecution {
    let status = match &outcome {
        ExecutionOutcome::Finished(_) => CoreSpaceExecutionStatus::Success,
        ExecutionOutcome::ExecutionErrorBumpNonce(_, _) => CoreSpaceExecutionStatus::Failed,
        ExecutionOutcome::NotExecutedDrop(_)
        | ExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
            CoreSpaceExecutionStatus::NotExecuted
        }
    };

    let failure = build_core_space_failure(&outcome);
    let executed = outcome.try_into_executed();

    let storage_collateralized = if status == CoreSpaceExecutionStatus::Success {
        executed
            .as_ref()
            .map(|executed| map_core_space_storage_changes(&executed.storage_collateralized))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let storage_released = if status == CoreSpaceExecutionStatus::Success {
        executed
            .as_ref()
            .map(|executed| map_core_space_storage_changes(&executed.storage_released))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let burnt_fee = if status == CoreSpaceExecutionStatus::NotExecuted {
        Some(U256::zero())
    } else {
        executed.as_ref().and_then(|executed| executed.burnt_fee)
    };

    CoreSpaceExecution {
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

pub(crate) fn build_core_space_not_executed(
    chain_id: u32,
    state: CoreSpaceStateAnchor,
    gas_limit: U256,
    failure: CoreSpaceExecutionFailure,
) -> CoreSpaceExecution {
    CoreSpaceExecution {
        chain_id: u64::from(chain_id),
        state,
        status: CoreSpaceExecutionStatus::NotExecuted,
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

fn map_core_space_storage_changes(changes: &[StorageChange]) -> Vec<CoreSpaceStorageChange> {
    changes
        .iter()
        .map(|change| CoreSpaceStorageChange {
            address: change.address,
            collateral_units: change.collaterals.as_u64(),
        })
        .collect()
}

fn build_core_space_failure(outcome: &ExecutionOutcome) -> Option<CoreSpaceExecutionFailure> {
    match outcome {
        ExecutionOutcome::Finished(_) => None,
        ExecutionOutcome::ExecutionErrorBumpNonce(error, executed) => Some(
            build_core_space_execution_error_failure(error, executed.output.as_ref()),
        ),
        ExecutionOutcome::NotExecutedDrop(error) => Some(build_core_space_drop_failure(error)),
        ExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
            Some(build_core_space_repack_failure(error))
        }
    }
}

fn build_core_space_drop_failure(error: &TxDropError) -> CoreSpaceExecutionFailure {
    match error {
        TxDropError::OldNonce(expected, got) => core_space_failure(
            CoreSpaceExecutionFailureCode::NonceTooLow,
            format!("transaction nonce {got} is lower than state nonce {expected}"),
        ),
        TxDropError::InvalidRecipientAddress(address) => core_space_failure(
            CoreSpaceExecutionFailureCode::InvalidRecipient,
            format!("invalid Core Space recipient address: {address:?}"),
        ),
        TxDropError::NotEnoughGasLimit { expected, got } => core_space_failure(
            CoreSpaceExecutionFailureCode::IntrinsicGasTooLow,
            format!("transaction gas limit {got} is lower than intrinsic gas {expected}"),
        ),
        TxDropError::SenderWithCode(address) => core_space_failure(
            CoreSpaceExecutionFailureCode::SenderWithCode,
            format!("transaction sender has contract code: {address:?}"),
        ),
    }
}

fn build_core_space_repack_failure(error: &ToRepackError) -> CoreSpaceExecutionFailure {
    match error {
        ToRepackError::InvalidNonce { expected, got } => core_space_failure(
            CoreSpaceExecutionFailureCode::NonceTooHigh,
            format!("transaction nonce {got} is higher than state nonce {expected}"),
        ),
        ToRepackError::EpochHeightOutOfBound {
            block_height,
            set,
            transaction_epoch_bound,
        } => core_space_failure(
            CoreSpaceExecutionFailureCode::EpochHeightOutOfBound,
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
        } => core_space_failure(
            CoreSpaceExecutionFailureCode::SponsorBalanceInsufficient,
            format!(
                "sponsor balance is insufficient: required gas \
                   {required_gas_cost}, available gas {gas_sponsor_balance}, \
                   required storage {required_storage_cost}, available storage \
                   {storage_sponsor_balance}"
            ),
        ),
        ToRepackError::SenderDoesNotExist => core_space_failure(
            CoreSpaceExecutionFailureCode::SenderDoesNotExist,
            "transaction sender does not exist",
        ),
        ToRepackError::NotEnoughBaseFee { expected, got } => core_space_failure(
            CoreSpaceExecutionFailureCode::FeeBelowBaseFee,
            format!(
                "transaction gas price {got} is lower than required base fee \
                   {expected}"
            ),
        ),
        ToRepackError::NotEnoughBalance { expected, got } => core_space_failure(
            CoreSpaceExecutionFailureCode::InsufficientFunds,
            format!("sender balance {got} is lower than required cost {expected}"),
        ),
    }
}

fn core_space_failure(
    code: CoreSpaceExecutionFailureCode,
    message: impl Into<String>,
) -> CoreSpaceExecutionFailure {
    CoreSpaceExecutionFailure {
        code,
        message: message.into(),
        reason: None,
    }
}

fn build_core_space_execution_error_failure(
    error: &ExecutionError,
    output: &[u8],
) -> CoreSpaceExecutionFailure {
    match error {
        ExecutionError::NotEnoughCash {
            required,
            got,
            actual_gas_cost,
            max_storage_limit_cost,
        } => core_space_failure(
            CoreSpaceExecutionFailureCode::InsufficientFunds,
            format!(
                "sender balance {got} is lower than required cost {required}; \
                   actual gas cost is {actual_gas_cost}, maximum storage cost is \
                   {max_storage_limit_cost}"
            ),
        ),
        ExecutionError::NonceOverflow(address) => core_space_failure(
            CoreSpaceExecutionFailureCode::NonceOverflow,
            format!("nonce overflow for address: {address:?}"),
        ),
        ExecutionError::VmError(error) => build_core_space_vm_failure(error, output),
    }
}

fn build_core_space_vm_failure(error: &vm::Error, output: &[u8]) -> CoreSpaceExecutionFailure {
    match error {
        vm::Error::Reverted => CoreSpaceExecutionFailure {
            code: CoreSpaceExecutionFailureCode::Revert,
            message: "execution reverted".to_string(),
            reason: revert_reason(output),
        },
        vm::Error::OutOfGas => core_space_failure(
            CoreSpaceExecutionFailureCode::OutOfGas,
            "execution ran out of gas",
        ),
        vm::Error::NotEnoughBalanceForStorage { required, got } => core_space_failure(
            CoreSpaceExecutionFailureCode::StorageBalanceInsufficient,
            format!(
                "storage collateral balance {got} is lower than required \
                       amount {required}"
            ),
        ),
        vm::Error::ExceedStorageLimit => core_space_failure(
            CoreSpaceExecutionFailureCode::StorageLimitExceeded,
            "execution exceeded the transaction storage limit",
        ),
        vm::Error::NonceOverflow(address) => core_space_failure(
            CoreSpaceExecutionFailureCode::NonceOverflow,
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
        | vm::Error::CreateContractStartingWithEF => core_space_failure(
            CoreSpaceExecutionFailureCode::VmError,
            format!("virtual machine execution failed: {error}"),
        ),
    }
}

fn revert_reason(_output: &[u8]) -> Option<String> {
    None
}

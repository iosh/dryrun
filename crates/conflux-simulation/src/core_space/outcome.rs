use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::Bytes;
use cfx_executor::executive::{ExecutionError, ToRepackError, TxDropError};
use cfx_types::{
    AddressSpaceUtil, CreateContractAddressType, Space, U512 as CfxU512,
    cal_contract_address_with_space,
};
use cfx_vm_types::Error as VmError;
use conflux_provider::{CoreAddress, Network};

use super::{
    CoreSpaceCompleteTransaction, CoreSpaceExecutionError, CoreSpaceExecutionFailure,
    CoreSpaceExecutionOutcome, CoreSpaceExecutionResult, CoreSpaceGas, CoreSpaceLog,
    CoreSpaceLogAddress, CoreSpaceResultIntegrationError, CoreSpaceRevertReason,
    CoreSpaceStateAccessError, CoreSpaceSuccessOutput, CoreSpaceTransactionRejection,
    ResolvedStorageSponsorship,
    executed_transaction::{CoreSpaceExecutedTransaction, CoreSpaceFinalStatus},
    state_access::CoreSpaceStateAccess,
};
use crate::primitive::{
    address_from_cfx, b256_from_cfx, u256_from_cfx, u256_to_cfx, u512_from_cfx,
};

pub(crate) fn build_execution_outcome(
    executed: CoreSpaceExecutedTransaction,
    transaction: &CoreSpaceCompleteTransaction,
    state: &CoreSpaceStateAccess,
    storage_sponsorship: Option<ResolvedStorageSponsorship>,
) -> Result<CoreSpaceExecutionOutcome, CoreSpaceExecutionError> {
    let common = transaction.common();
    let network = common.from.network();
    let storage_outcome = match executed.status {
        CoreSpaceFinalStatus::Success => StorageCoverageOutcome::Success,
        CoreSpaceFinalStatus::Reverted => StorageCoverageOutcome::Reverted,
        CoreSpaceFinalStatus::Failed(_) => StorageCoverageOutcome::FullyChargedFailure,
    };
    let storage_covered_by_sponsor = storage_covered_by_sponsor_for_outcome(
        storage_sponsorship,
        executed.storage_sponsor_paid,
        storage_outcome,
        executed.cip78a,
        executed.cip78b,
    )?;
    let result = build_execution_result(&executed, common.gas_limit, storage_covered_by_sponsor)?;

    match executed.status {
        CoreSpaceFinalStatus::Success => {
            let logs = map_committed_logs(&executed.committed_logs, network)?;
            let output = build_success_output(&executed, transaction, state, network)?;
            Ok(CoreSpaceExecutionOutcome::Success {
                result,
                output,
                logs,
            })
        }
        CoreSpaceFinalStatus::Reverted => {
            let reason = decode_revert_reason(&executed.output);
            Ok(CoreSpaceExecutionOutcome::Reverted {
                result,
                revert_data: executed.output,
                reason,
            })
        }
        CoreSpaceFinalStatus::Failed(failure) => Ok(CoreSpaceExecutionOutcome::Failed {
            result,
            failure: map_execution_failure(failure, network)?,
        }),
    }
}

fn build_execution_result(
    executed: &CoreSpaceExecutedTransaction,
    gas_limit: alloy_primitives::U256,
    storage_covered_by_sponsor: bool,
) -> Result<CoreSpaceExecutionResult, CoreSpaceResultIntegrationError> {
    let gas = CoreSpaceGas::new(
        gas_limit,
        executed.intrinsic_gas,
        executed.gas_used,
        executed.gas_charged,
    )?;
    Ok(CoreSpaceExecutionResult::new(
        gas,
        executed.gas_fee,
        executed.burnt_gas_fee,
        executed.effective_gas_price,
        executed.gas_sponsor_paid,
        executed.storage_collateralized,
        storage_covered_by_sponsor,
    ))
}

fn build_success_output(
    executed: &CoreSpaceExecutedTransaction,
    transaction: &CoreSpaceCompleteTransaction,
    state: &CoreSpaceStateAccess,
    network: Network,
) -> Result<CoreSpaceSuccessOutput, CoreSpaceExecutionError> {
    let common = transaction.common();
    if common.to.is_some() {
        return Ok(CoreSpaceSuccessOutput::Call {
            return_data: executed.output.clone(),
        });
    }

    let sender = executed.sender.with_native_space();
    let (created, _) = cal_contract_address_with_space(
        CreateContractAddressType::FromSenderNonceAndCodeHash,
        &sender,
        &u256_to_cfx(common.nonce),
        common.data.as_ref(),
    );
    let created_address = core_address(created.address, network)?;
    if !executed.contracts_created.contains(&created) {
        return Err(CoreSpaceResultIntegrationError::MissingCreatedContract {
            address: created_address,
        }
        .into());
    }

    let runtime_code = state.finalized().code(created)?.unwrap_or_default();

    Ok(CoreSpaceSuccessOutput::Create {
        address: created_address,
        runtime_code,
    })
}

fn map_committed_logs(
    logs: &[primitives::LogEntry],
    network: Network,
) -> Result<Vec<CoreSpaceLog>, CoreSpaceResultIntegrationError> {
    logs.iter()
        .map(|log| {
            let address = match log.space {
                Space::Native => {
                    CoreSpaceLogAddress::CoreSpace(core_address(log.address, network)?)
                }
                Space::Ethereum => CoreSpaceLogAddress::Espace(address_from_cfx(log.address)),
            };
            Ok(CoreSpaceLog {
                address,
                topics: log.topics.iter().copied().map(b256_from_cfx).collect(),
                data: Bytes::copy_from_slice(log.data.as_ref()),
            })
        })
        .collect()
}

pub(crate) fn map_drop_error(
    error: TxDropError,
    network: Network,
) -> Result<CoreSpaceTransactionRejection, CoreSpaceResultIntegrationError> {
    match error {
        TxDropError::OldNonce(expected, got) => Ok(CoreSpaceTransactionRejection::NonceTooLow {
            transaction_nonce: u256_from_cfx(got),
            state_nonce: u256_from_cfx(expected),
        }),
        TxDropError::InvalidRecipientAddress(recipient) => {
            Ok(CoreSpaceTransactionRejection::InvalidRecipient {
                recipient: core_address(recipient, network)?,
            })
        }
        TxDropError::NotEnoughGasLimit { expected, got } => {
            Ok(CoreSpaceTransactionRejection::IntrinsicGasExceedsGasLimit {
                intrinsic_gas: u256_from_cfx(expected),
                gas_limit: u256_from_cfx(got),
            })
        }
        TxDropError::SenderWithCode(sender) => Ok(CoreSpaceTransactionRejection::SenderHasCode {
            sender: core_address(sender, network)?,
        }),
    }
}

pub(crate) fn map_reconsider_packing_error(
    error: ToRepackError,
) -> Result<CoreSpaceTransactionRejection, CoreSpaceResultIntegrationError> {
    match error {
        ToRepackError::InvalidNonce { expected, got } if got < expected => {
            Ok(CoreSpaceTransactionRejection::NonceTooLow {
                transaction_nonce: u256_from_cfx(got),
                state_nonce: u256_from_cfx(expected),
            })
        }
        ToRepackError::InvalidNonce { expected, got } if got > expected => {
            Ok(CoreSpaceTransactionRejection::NonceTooHigh {
                transaction_nonce: u256_from_cfx(got),
                state_nonce: u256_from_cfx(expected),
            })
        }
        ToRepackError::InvalidNonce { expected, got } => Err(
            CoreSpaceResultIntegrationError::invalid_executor_output(format!(
                "executor rejected equal transaction and state nonces: expected {expected}, got {got}"
            )),
        ),
        ToRepackError::EpochHeightOutOfBound {
            block_height,
            set,
            transaction_epoch_bound,
        } => Ok(CoreSpaceTransactionRejection::EpochHeightOutOfBounds {
            execution_epoch_height: block_height,
            transaction_epoch_height: set,
            epoch_bound: transaction_epoch_bound,
        }),
        ToRepackError::NotEnoughCashFromSponsor {
            required_gas_cost,
            gas_sponsor_balance,
            required_storage_cost,
            storage_sponsor_balance,
        } => Ok(CoreSpaceTransactionRejection::SponsorBalanceInsufficient {
            required_gas_cost: u512_from_cfx(required_gas_cost),
            available_gas_balance: u512_from_cfx(gas_sponsor_balance),
            required_storage_cost: u256_from_cfx(required_storage_cost),
            available_storage_balance: u256_from_cfx(storage_sponsor_balance),
        }),
        ToRepackError::SenderDoesNotExist => Ok(CoreSpaceTransactionRejection::SenderDoesNotExist),
        ToRepackError::NotEnoughBaseFee { expected, got } => {
            Ok(CoreSpaceTransactionRejection::GasPriceBelowBaseFee {
                gas_price: u256_from_cfx(got),
                base_fee_per_gas: u256_from_cfx(expected),
            })
        }
        ToRepackError::NotEnoughBalance { expected, got } => {
            Ok(CoreSpaceTransactionRejection::InsufficientFunds {
                required: u512_from_cfx(expected),
                available: u512_from_cfx(CfxU512::from(got)),
            })
        }
    }
}

fn map_execution_failure(
    error: ExecutionError,
    network: Network,
) -> Result<CoreSpaceExecutionFailure, CoreSpaceExecutionError> {
    match error {
        ExecutionError::NotEnoughCash {
            required,
            got,
            actual_gas_cost,
            max_storage_limit_cost,
        } => Ok(CoreSpaceExecutionFailure::InsufficientFunds {
            required: u512_from_cfx(required),
            available: u512_from_cfx(got),
            actual_gas_cost: u256_from_cfx(actual_gas_cost),
            maximum_storage_cost: u256_from_cfx(max_storage_limit_cost),
        }),
        ExecutionError::NonceOverflow(address) => Ok(CoreSpaceExecutionFailure::NonceOverflow {
            address: core_address(address, network)?,
        }),
        ExecutionError::VmError(error) => map_vm_failure(error, network),
    }
}

fn map_vm_failure(
    error: VmError,
    network: Network,
) -> Result<CoreSpaceExecutionFailure, CoreSpaceExecutionError> {
    match error {
        VmError::OutOfGas => Ok(CoreSpaceExecutionFailure::OutOfGas),
        VmError::BadJumpDestination { destination } => {
            Ok(CoreSpaceExecutionFailure::InvalidJump { destination })
        }
        VmError::BadInstruction { instruction } => {
            Ok(CoreSpaceExecutionFailure::InvalidInstruction { instruction })
        }
        VmError::StackUnderflow {
            instruction,
            wanted,
            on_stack,
        } => Ok(CoreSpaceExecutionFailure::StackUnderflow {
            instruction,
            wanted,
            available: on_stack,
        }),
        VmError::OutOfStack {
            instruction,
            wanted,
            limit,
        } => Ok(CoreSpaceExecutionFailure::StackOverflow {
            instruction,
            wanted,
            limit,
        }),
        VmError::SubStackUnderflow { wanted, on_stack } => {
            Ok(CoreSpaceExecutionFailure::SubroutineStackUnderflow {
                wanted,
                available: on_stack,
            })
        }
        VmError::OutOfSubStack { wanted, limit } => {
            Ok(CoreSpaceExecutionFailure::SubroutineStackOverflow { wanted, limit })
        }
        VmError::InvalidSubEntry => Ok(CoreSpaceExecutionFailure::InvalidSubroutineEntry),
        VmError::NotEnoughBalanceForStorage { required, got } => {
            Ok(CoreSpaceExecutionFailure::StorageBalanceInsufficient {
                required: u256_from_cfx(required),
                available: u256_from_cfx(got),
            })
        }
        VmError::ExceedStorageLimit => Ok(CoreSpaceExecutionFailure::StorageLimitExceeded),
        VmError::BuiltIn(details) => Ok(CoreSpaceExecutionFailure::BuiltInContract { details }),
        VmError::InternalContract(details) => {
            Ok(CoreSpaceExecutionFailure::InternalContract { details })
        }
        VmError::MutableCallInStaticContext => {
            Ok(CoreSpaceExecutionFailure::StateChangeDuringStaticCall)
        }
        VmError::CreateInitCodeSizeLimit => Ok(CoreSpaceExecutionFailure::CreateInitCodeSizeLimit),
        VmError::StateDbError(error) => Err(CoreSpaceStateAccessError::Operation {
            operation: "execute Core Space transaction",
            source: error.0,
        }
        .into()),
        VmError::Wasm(details) => Ok(CoreSpaceExecutionFailure::Wasm { details }),
        VmError::OutOfBounds => Ok(CoreSpaceExecutionFailure::ReturnDataOutOfBounds),
        VmError::Reverted => Err(CoreSpaceResultIntegrationError::invalid_executor_output(
            "revert reached the non-revert Core Space failure classifier",
        )
        .into()),
        VmError::InvalidAddress(address) => Ok(CoreSpaceExecutionFailure::InvalidAddress {
            address: core_address(address, network)?,
        }),
        VmError::ConflictAddress(address) => Ok(CoreSpaceExecutionFailure::CreateCollision {
            address: core_address(address, network)?,
        }),
        VmError::NonceOverflow(address) => Ok(CoreSpaceExecutionFailure::NonceOverflow {
            address: core_address(address, network)?,
        }),
        VmError::CreateContractStartingWithEF => {
            Ok(CoreSpaceExecutionFailure::CreateContractStartingWithEf)
        }
    }
}

fn decode_revert_reason(output: &Bytes) -> Option<CoreSpaceRevertReason> {
    Revert::abi_decode_validate(output.as_ref())
        .map(|revert| CoreSpaceRevertReason::SolidityError {
            message: revert.reason,
        })
        .or_else(|_| {
            Panic::abi_decode_validate(output.as_ref())
                .map(|panic| CoreSpaceRevertReason::SolidityPanic { code: panic.code })
        })
        .ok()
}

fn core_address(
    address: cfx_types::Address,
    network: Network,
) -> Result<CoreAddress, CoreSpaceResultIntegrationError> {
    CoreAddress::from_bytes(*address.as_fixed_bytes(), network).map_err(|error| {
        CoreSpaceResultIntegrationError::InvalidCoreAddress {
            details: error.to_string(),
        }
    })
}

#[derive(Clone, Copy)]
enum StorageCoverageOutcome {
    Success,
    Reverted,
    FullyChargedFailure,
}

fn storage_covered_by_sponsor_for_outcome(
    resolved: Option<ResolvedStorageSponsorship>,
    executor_reported: bool,
    outcome: StorageCoverageOutcome,
    cip78a: bool,
    cip78b: bool,
) -> Result<bool, CoreSpaceResultIntegrationError> {
    let use_prepared_value = match outcome {
        StorageCoverageOutcome::Success | StorageCoverageOutcome::Reverted => cip78a,
        StorageCoverageOutcome::FullyChargedFailure => cip78b,
    };
    // EstimateSender cannot report post-CIP-78 storage sponsorship. Reconstruct
    // normal receipt semantics from the same anchored state in those branches.
    if use_prepared_value {
        resolved
            .map(ResolvedStorageSponsorship::storage_covered_by_sponsor)
            .ok_or_else(|| {
                CoreSpaceResultIntegrationError::invalid_executor_output(
                    "CIP-78 storage sponsorship was not resolved",
                )
            })
    } else {
        Ok(executor_reported)
    }
}

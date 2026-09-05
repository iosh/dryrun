use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::Bytes;
use cfx_executor::executive::{ExecutionError, ToRepackError, TxDropError};
use cfx_types::{
    AddressSpaceUtil, CreateContractAddressType, U256 as CfxU256, U512 as CfxU512,
    cal_contract_address_with_space,
};
use cfx_vm_types::Error as VmError;
use conflux_provider::Network;

use super::{
    EspaceCompleteTransaction, EspaceExecutedTransaction, EspaceExecutionError,
    EspaceExecutionFailure, EspaceExecutionOutcome, EspaceExecutionResult, EspaceExecutionSpace,
    EspaceFee, EspaceGas, EspaceLog, EspaceLogAddress, EspaceResultIntegrationError,
    EspaceRevertReason, EspaceStateAccessError, EspaceStateReadError, EspaceStateReader,
    EspaceSuccessOutput, EspaceTransactionRejection,
};
use crate::{
    execution::{ConfluxExecutionOutcome, ConfluxExecutionOutput, PreparedTransactionExecution},
    primitive::{address_from_cfx, address_to_cfx, u256_from_cfx, u512_from_cfx},
};

pub(crate) fn convert_executor_outcome(
    outcome: ConfluxExecutionOutcome,
    execution: Option<&EspaceExecutedTransaction>,
    prepared: &PreparedTransactionExecution,
    transaction: &EspaceCompleteTransaction,
    state: Option<&EspaceStateReader>,
    core_space_network: Network,
) -> Result<EspaceExecutionOutcome, EspaceExecutionError> {
    let common = transaction.common();
    match outcome {
        ConfluxExecutionOutcome::Success(output) => {
            let execution = execution.ok_or_else(|| {
                EspaceResultIntegrationError::new(
                    "successful transaction has no finalized executed-transaction data",
                )
            })?;
            let result = build_execution_result(&output, common.gas_limit)?;
            let logs = convert_committed_logs(execution, core_space_network)?;
            let output = build_success_output(execution, &output, prepared, transaction, state)?;
            Ok(EspaceExecutionOutcome::Success {
                result,
                output,
                logs,
            })
        }
        ConfluxExecutionOutcome::Failed { error, details } => {
            let result = build_execution_result(&details, common.gas_limit)?;
            let return_data = details.common.output.clone();
            match error {
                ExecutionError::VmError(VmError::Reverted) => {
                    let reason = decode_revert_reason(&return_data);
                    Ok(EspaceExecutionOutcome::Reverted {
                        result,
                        revert_data: return_data,
                        reason,
                    })
                }
                error => Ok(EspaceExecutionOutcome::Failed {
                    result,
                    failure: classify_execution_failure(error)?,
                }),
            }
        }
        ConfluxExecutionOutcome::NotExecutedDrop(error) => Ok(EspaceExecutionOutcome::NotExecuted(
            classify_drop_rejection(error)?,
        )),
        ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(error) => Ok(
            EspaceExecutionOutcome::NotExecuted(classify_repack_rejection(error)?),
        ),
    }
}

fn build_execution_result(
    output: &ConfluxExecutionOutput,
    gas_limit: u64,
) -> Result<EspaceExecutionResult, EspaceResultIntegrationError> {
    let gas = EspaceGas::new(
        gas_limit,
        output.base_gas,
        output.common.gas_used,
        output.common.gas_charged,
    )?;
    let fee = EspaceFee::new(output.common.fee, output.common.burnt_fee)?;
    Ok(EspaceExecutionResult::new(gas, fee))
}

fn build_success_output(
    execution: &EspaceExecutedTransaction,
    output: &ConfluxExecutionOutput,
    _prepared: &PreparedTransactionExecution,
    transaction: &EspaceCompleteTransaction,
    state: Option<&EspaceStateReader>,
) -> Result<EspaceSuccessOutput, EspaceExecutionError> {
    let common = transaction.common();
    if common.to.is_some() {
        return Ok(EspaceSuccessOutput::Call {
            return_data: output.common.output.clone(),
        });
    }

    let sender = address_to_cfx(common.from).with_evm_space();
    let nonce = CfxU256::from(common.nonce);
    let (created, _) = cal_contract_address_with_space(
        CreateContractAddressType::FromSenderNonce,
        &sender,
        &nonce,
        common.input.as_ref(),
    );
    let created_address = address_from_cfx(created.address);
    if !execution.contracts_created().iter().any(|contract| {
        contract.space() == EspaceExecutionSpace::Espace && contract.address() == created_address
    }) {
        return Err(EspaceResultIntegrationError::new(format!(
            "successful contract creation did not report the expected address {}",
            created_address
        ))
        .into());
    }

    let state = state.ok_or_else(|| {
        EspaceResultIntegrationError::invalid_executor_output(
            "successful eSpace contract creation has no finalized state reader",
        )
    })?;
    let runtime_code = state
        .read_account(created_address)
        .map_err(map_state_read_error)?
        .code()
        .cloned()
        .unwrap_or_default();

    Ok(EspaceSuccessOutput::Create {
        address: created_address,
        runtime_code,
    })
}

fn map_state_read_error(error: EspaceStateReadError) -> EspaceExecutionError {
    match error {
        EspaceStateReadError::StateAccess(source) => EspaceExecutionError::StateAccess(source),
        error => EspaceResultIntegrationError::invalid_executor_output(format!(
            "created eSpace contract code could not be read from finalized state: {error}"
        ))
        .into(),
    }
}

fn convert_committed_logs(
    execution: &EspaceExecutedTransaction,
    core_space_network: Network,
) -> Result<Vec<EspaceLog>, EspaceResultIntegrationError> {
    execution
        .committed_logs()
        .iter()
        .map(|log| {
            let address = match log.space() {
                EspaceExecutionSpace::Espace => EspaceLogAddress::Espace(log.address()),
                EspaceExecutionSpace::Core => {
                    let bytes = log.address().into_array();
                    let address =
                        conflux_provider::CoreAddress::from_bytes(bytes, core_space_network)
                            .map_err(|source| {
                                EspaceResultIntegrationError::new(format!(
                                    "failed to represent a committed Conflux log address: {source}"
                                ))
                            })?;
                    EspaceLogAddress::CoreSpace(address)
                }
            };
            Ok(EspaceLog {
                address,
                topics: log.topics().to_vec(),
                data: log.data().clone(),
            })
        })
        .collect()
}

fn classify_drop_rejection(
    error: TxDropError,
) -> Result<EspaceTransactionRejection, EspaceExecutionError> {
    match error {
        TxDropError::OldNonce(expected, got) => Ok(EspaceTransactionRejection::NonceTooLow {
            transaction_nonce: u256_from_cfx(got),
            state_nonce: u256_from_cfx(expected),
        }),
        TxDropError::NotEnoughGasLimit { expected, got } => {
            Ok(EspaceTransactionRejection::IntrinsicGasExceedsGasLimit {
                intrinsic_gas: u256_from_cfx(expected),
                gas_limit: u256_from_cfx(got),
            })
        }
        TxDropError::SenderWithCode(sender) => Ok(EspaceTransactionRejection::SenderHasCode {
            sender: address_from_cfx(sender),
        }),
        TxDropError::InvalidRecipientAddress(address) => Err(
            EspaceResultIntegrationError::invalid_executor_output(format!(
                "eSpace transaction produced Core Space-only invalid recipient {address:?}"
            ))
            .into(),
        ),
    }
}

fn classify_repack_rejection(
    error: ToRepackError,
) -> Result<EspaceTransactionRejection, EspaceExecutionError> {
    match error {
        ToRepackError::InvalidNonce { expected, got } if got < expected => {
            Ok(EspaceTransactionRejection::NonceTooLow {
                transaction_nonce: u256_from_cfx(got),
                state_nonce: u256_from_cfx(expected),
            })
        }
        ToRepackError::InvalidNonce { expected, got } if got > expected => {
            Ok(EspaceTransactionRejection::NonceTooHigh {
                transaction_nonce: u256_from_cfx(got),
                state_nonce: u256_from_cfx(expected),
            })
        }
        ToRepackError::InvalidNonce { expected, got } => {
            Err(EspaceResultIntegrationError::invalid_executor_output(format!(
                "executor rejected equal transaction and state nonces: expected {expected}, got {got}"
            ))
            .into())
        }
        ToRepackError::SenderDoesNotExist => Ok(EspaceTransactionRejection::SenderDoesNotExist),
        ToRepackError::NotEnoughBaseFee { expected, got } => {
            Ok(EspaceTransactionRejection::GasPriceBelowBaseFee {
                gas_price: u256_from_cfx(got),
                base_fee_per_gas: u256_from_cfx(expected),
            })
        }
        ToRepackError::NotEnoughBalance { expected, got } => {
            Ok(EspaceTransactionRejection::InsufficientFunds {
                required: u512_from_cfx(expected),
                available: u512_from_cfx(CfxU512::from(got)),
            })
        }
        ToRepackError::EpochHeightOutOfBound { .. }
        | ToRepackError::NotEnoughCashFromSponsor { .. } => {
            Err(EspaceResultIntegrationError::invalid_executor_output(format!(
                "eSpace transaction produced Core Space-only rejection {error:?}"
            ))
            .into())
        }
    }
}

fn classify_execution_failure(
    error: ExecutionError,
) -> Result<EspaceExecutionFailure, EspaceExecutionError> {
    match error {
        ExecutionError::NotEnoughCash {
            required,
            got,
            actual_gas_cost,
            max_storage_limit_cost,
        } => Ok(EspaceExecutionFailure::InsufficientFunds {
            required: u512_from_cfx(required),
            available: u512_from_cfx(got),
            actual_gas_cost: u256_from_cfx(actual_gas_cost),
            maximum_storage_cost: u256_from_cfx(max_storage_limit_cost),
        }),
        ExecutionError::NonceOverflow(address) => Ok(EspaceExecutionFailure::NonceOverflow {
            address: address_from_cfx(address),
        }),
        ExecutionError::VmError(error) => classify_vm_failure(error),
    }
}

fn classify_vm_failure(error: VmError) -> Result<EspaceExecutionFailure, EspaceExecutionError> {
    match error {
        VmError::OutOfGas => Ok(EspaceExecutionFailure::OutOfGas),
        VmError::BadJumpDestination { destination } => {
            Ok(EspaceExecutionFailure::InvalidJump { destination })
        }
        VmError::BadInstruction { instruction } => {
            Ok(EspaceExecutionFailure::InvalidInstruction { instruction })
        }
        VmError::StackUnderflow {
            instruction,
            wanted,
            on_stack,
        } => Ok(EspaceExecutionFailure::StackUnderflow {
            instruction,
            wanted,
            available: on_stack,
        }),
        VmError::OutOfStack {
            instruction,
            wanted,
            limit,
        } => Ok(EspaceExecutionFailure::StackOverflow {
            instruction,
            wanted,
            limit,
        }),
        VmError::SubStackUnderflow { wanted, on_stack } => {
            Ok(EspaceExecutionFailure::SubroutineStackUnderflow {
                wanted,
                available: on_stack,
            })
        }
        VmError::OutOfSubStack { wanted, limit } => {
            Ok(EspaceExecutionFailure::SubroutineStackOverflow { wanted, limit })
        }
        VmError::InvalidSubEntry => Ok(EspaceExecutionFailure::InvalidSubroutineEntry),
        VmError::BuiltIn(details) => Ok(EspaceExecutionFailure::BuiltInContract { details }),
        VmError::InternalContract(details) => {
            Ok(EspaceExecutionFailure::InternalContract { details })
        }
        VmError::MutableCallInStaticContext => {
            Ok(EspaceExecutionFailure::StateChangeDuringStaticCall)
        }
        VmError::CreateInitCodeSizeLimit => Ok(EspaceExecutionFailure::CreateInitCodeSizeLimit),
        VmError::OutOfBounds => Ok(EspaceExecutionFailure::ReturnDataOutOfBounds),
        VmError::InvalidAddress(address) => Ok(EspaceExecutionFailure::InvalidAddress {
            address: address_from_cfx(address),
        }),
        VmError::ConflictAddress(address) => Ok(EspaceExecutionFailure::CreateCollision {
            address: address_from_cfx(address),
        }),
        VmError::NonceOverflow(address) => Ok(EspaceExecutionFailure::NonceOverflow {
            address: address_from_cfx(address),
        }),
        VmError::CreateContractStartingWithEF => {
            Ok(EspaceExecutionFailure::CreateContractStartingWithEf)
        }
        VmError::StateDbError(error) => Err(EspaceStateAccessError::Operation {
            operation: "execute eSpace transaction",
            source: error.0,
        }
        .into()),
        VmError::NotEnoughBalanceForStorage { .. } | VmError::ExceedStorageLimit => Err(
            EspaceResultIntegrationError::invalid_executor_output(format!(
                "eSpace transaction produced Core Space-only VM failure: {error}"
            ))
            .into(),
        ),
        VmError::Wasm(details) => Err(EspaceResultIntegrationError::invalid_executor_output(
            format!("eSpace transaction produced unsupported Wasm failure: {details}"),
        )
        .into()),
        VmError::Reverted => Err(EspaceResultIntegrationError::invalid_executor_output(
            "revert reached the non-revert VM failure classifier",
        )
        .into()),
    }
}

fn decode_revert_reason(output: &Bytes) -> Option<EspaceRevertReason> {
    Revert::abi_decode_validate(output.as_ref())
        .map(|revert| EspaceRevertReason::SolidityError {
            message: revert.reason,
        })
        .or_else(|_| {
            Panic::abi_decode_validate(output.as_ref())
                .map(|panic| EspaceRevertReason::SolidityPanic { code: panic.code })
        })
        .ok()
}

use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::Bytes;
use revm::context_interface::result::{
    ExecutionResult as RevmExecutionResult, HaltReason, OutOfGasError, Output as RevmOutput,
    SuccessReason,
};

use crate::{
    CompleteTransaction, EvmExecutionError, EvmExecutionOutcome, EvmExecutionResult, EvmHaltReason,
    EvmOutOfGasReason, EvmResultIntegrationError, EvmRevertReason, EvmSuccessOutput,
    EvmSuccessReason,
};

pub(crate) fn map_executed_outcome(
    result: RevmExecutionResult<HaltReason>,
    transaction: &CompleteTransaction,
    execution_result: EvmExecutionResult,
) -> Result<EvmExecutionOutcome, EvmExecutionError> {
    match result {
        RevmExecutionResult::Success {
            reason,
            logs,
            output,
            ..
        } => Ok(EvmExecutionOutcome::Success {
            result: execution_result,
            reason: map_success_reason(reason),
            output: map_success_output(output, transaction)?,
            logs,
        }),
        RevmExecutionResult::Revert { output, .. } => {
            let reason = decode_revert_reason(&output);
            Ok(EvmExecutionOutcome::Reverted {
                result: execution_result,
                revert_data: output,
                reason,
            })
        }
        RevmExecutionResult::Halt { reason, .. } => Ok(EvmExecutionOutcome::Halted {
            result: execution_result,
            reason: map_halt_reason(reason),
        }),
    }
}

const fn map_success_reason(reason: SuccessReason) -> EvmSuccessReason {
    match reason {
        SuccessReason::Stop => EvmSuccessReason::Stop,
        SuccessReason::Return => EvmSuccessReason::Return,
        SuccessReason::SelfDestruct => EvmSuccessReason::SelfDestruct,
    }
}

fn map_success_output(
    output: RevmOutput,
    transaction: &CompleteTransaction,
) -> Result<EvmSuccessOutput, EvmExecutionError> {
    match (transaction.to, output) {
        (Some(_), RevmOutput::Call(return_data)) => Ok(EvmSuccessOutput::Call { return_data }),
        (None, RevmOutput::Create(runtime_code, Some(address))) => Ok(EvmSuccessOutput::Create {
            address,
            runtime_code,
        }),
        (Some(_), RevmOutput::Create(_, _)) => {
            Err(EvmResultIntegrationError::CreateOutputForCall.into())
        }
        (None, RevmOutput::Call(_)) => Err(EvmResultIntegrationError::CallOutputForCreate.into()),
        (None, RevmOutput::Create(_, None)) => {
            Err(EvmResultIntegrationError::MissingCreateAddress.into())
        }
    }
}

fn decode_revert_reason(output: &Bytes) -> Option<EvmRevertReason> {
    Revert::abi_decode_validate(output.as_ref())
        .map(|revert| EvmRevertReason::SolidityError {
            message: revert.reason,
        })
        .or_else(|_| {
            Panic::abi_decode_validate(output.as_ref())
                .map(|panic| EvmRevertReason::SolidityPanic { code: panic.code })
        })
        .ok()
}

fn map_halt_reason(reason: HaltReason) -> EvmHaltReason {
    match reason {
        HaltReason::OutOfGas(reason) => EvmHaltReason::OutOfGas(map_out_of_gas_reason(reason)),
        HaltReason::OpcodeNotFound => EvmHaltReason::OpcodeNotFound,
        HaltReason::InvalidFEOpcode => EvmHaltReason::InvalidFeOpcode,
        HaltReason::InvalidJump => EvmHaltReason::InvalidJump,
        HaltReason::NotActivated => EvmHaltReason::NotActivated,
        HaltReason::StackUnderflow => EvmHaltReason::StackUnderflow,
        HaltReason::StackOverflow => EvmHaltReason::StackOverflow,
        HaltReason::OutOfOffset => EvmHaltReason::OutOfOffset,
        HaltReason::CreateCollision => EvmHaltReason::CreateCollision,
        HaltReason::PrecompileError => EvmHaltReason::PrecompileError,
        HaltReason::PrecompileErrorWithContext(message) => {
            EvmHaltReason::PrecompileErrorWithContext { message }
        }
        HaltReason::NonceOverflow => EvmHaltReason::NonceOverflow,
        HaltReason::CreateContractSizeLimit => EvmHaltReason::CreateContractSizeLimit,
        HaltReason::CreateContractStartingWithEF => EvmHaltReason::CreateContractStartingWithEf,
        HaltReason::CreateInitCodeSizeLimit => EvmHaltReason::CreateInitCodeSizeLimit,
        HaltReason::OverflowPayment => EvmHaltReason::PaymentOverflow,
        HaltReason::StateChangeDuringStaticCall => EvmHaltReason::StateChangeDuringStaticCall,
        HaltReason::CallNotAllowedInsideStatic => EvmHaltReason::CallNotAllowedInsideStatic,
        HaltReason::OutOfFunds => EvmHaltReason::OutOfFunds,
        HaltReason::CallTooDeep => EvmHaltReason::CallTooDeep,
    }
}

const fn map_out_of_gas_reason(reason: OutOfGasError) -> EvmOutOfGasReason {
    match reason {
        OutOfGasError::Basic => EvmOutOfGasReason::Basic,
        OutOfGasError::MemoryLimit => EvmOutOfGasReason::MemoryLimit,
        OutOfGasError::Memory => EvmOutOfGasReason::MemoryExpansion,
        OutOfGasError::Precompile => EvmOutOfGasReason::Precompile,
        OutOfGasError::InvalidOperand => EvmOutOfGasReason::InvalidOperand,
        OutOfGasError::ReentrancySentry => EvmOutOfGasReason::ReentrancySentry,
    }
}

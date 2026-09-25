use alloy_primitives::Bytes;
use revm::context_interface::result::{
    ExecutionResult as RevmExecutionResult, HaltReason, OutOfGasError, Output as RevmOutput,
    SuccessReason,
};

use crate::{
    EvmExecutionError, EvmHaltReason, EvmOutOfGasReason, EvmResultIntegrationError,
    EvmRevertReason, EvmSuccessOutput, EvmSuccessReason, TypedTransaction,
};

#[derive(Debug)]
pub(crate) enum EvmFinalStatus {
    Success {
        reason: EvmSuccessReason,
        output: EvmSuccessOutput,
    },
    Reverted {
        revert_data: Bytes,
        reason: Option<EvmRevertReason>,
    },
    Halted {
        reason: EvmHaltReason,
    },
}

pub(crate) fn map_executed_status(
    result: RevmExecutionResult<HaltReason>,
    transaction: &TypedTransaction,
) -> Result<EvmFinalStatus, EvmExecutionError> {
    match result {
        RevmExecutionResult::Success { reason, output, .. } => Ok(EvmFinalStatus::Success {
            reason: map_success_reason(reason),
            output: map_success_output(output, transaction)?,
        }),
        RevmExecutionResult::Revert { output, .. } => {
            let reason = EvmRevertReason::decode(&output);
            Ok(EvmFinalStatus::Reverted {
                revert_data: output,
                reason,
            })
        }
        RevmExecutionResult::Halt { reason, .. } => Ok(EvmFinalStatus::Halted {
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
    transaction: &TypedTransaction,
) -> Result<EvmSuccessOutput, EvmExecutionError> {
    let common = transaction.common();
    match (common.to, output) {
        (Some(_), RevmOutput::Call(return_data)) => Ok(EvmSuccessOutput::Call { return_data }),
        (None, RevmOutput::Create(runtime_code, Some(address))) => Ok(EvmSuccessOutput::Create {
            address,
            runtime_code,
        }),
        (Some(_), RevmOutput::Create(_, _)) => Err(EvmResultIntegrationError::new(
            "execution engine returned create output for a call transaction",
        )
        .into()),
        (None, RevmOutput::Call(_)) => Err(EvmResultIntegrationError::new(
            "execution engine returned call output for a contract creation transaction",
        )
        .into()),
        (None, RevmOutput::Create(_, None)) => Err(EvmResultIntegrationError::new(
            "successful contract creation did not return the created address",
        )
        .into()),
    }
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

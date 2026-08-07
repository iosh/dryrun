use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    sol_types::{Panic, Revert, SolError},
};
use alloy_primitives::Bytes;
use revm::context_interface::result::{ExecutionResult, HaltReason, InvalidTransaction};
use simulation_transaction::Transaction;

use crate::{
    EvmBlockContext, EvmExecution, EvmExecutionDetails, EvmExecutionFailure,
    EvmExecutionFailureCode, EvmFeeSettlement, EvmOutcome,
};

pub(crate) fn build_execution(
    result: ExecutionResult<HaltReason>,
    chain_id: u64,
    block: &Sealed<Header>,
    fee_settlement: &EvmFeeSettlement,
) -> EvmExecution {
    match result {
        ExecutionResult::Success { gas, output, .. } => EvmExecution {
            chain_id,
            context: simulated_block(block),
            gas_limit: gas.limit(),
            outcome: EvmOutcome::Success(EvmExecutionDetails {
                gas_used: gas.used(),
                gas_charged: gas.used(),
                fee: fee_settlement.fee,
                burnt_fee: fee_settlement.burnt_fee,
                output: output.into_data(),
            }),
        },
        ExecutionResult::Revert { gas, output, .. } => build_revert_execution(
            chain_id,
            block,
            gas.used(),
            gas.limit(),
            output,
            fee_settlement,
        ),
        ExecutionResult::Halt { reason, gas, .. } => build_halt_execution(
            chain_id,
            block,
            gas.used(),
            gas.limit(),
            reason,
            fee_settlement,
        ),
    }
}

pub(crate) fn build_not_executed(
    chain_id: u64,
    block: &Sealed<Header>,
    transaction: &Transaction,
    error: InvalidTransaction,
) -> EvmExecution {
    EvmExecution {
        chain_id,
        context: simulated_block(block),
        gas_limit: transaction.gas_limit,
        outcome: EvmOutcome::NotExecuted(build_invalid_transaction_failure(error)),
    }
}

fn build_revert_execution(
    chain_id: u64,
    block: &Sealed<Header>,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    fee_settlement: &EvmFeeSettlement,
) -> EvmExecution {
    let failure = build_revert_failure(&output);

    build_failed_execution(
        chain_id,
        block,
        gas_used,
        gas_limit,
        output,
        failure,
        fee_settlement,
    )
}

fn build_halt_execution(
    chain_id: u64,
    block: &Sealed<Header>,
    gas_used: u64,
    gas_limit: u64,
    reason: HaltReason,
    fee_settlement: &EvmFeeSettlement,
) -> EvmExecution {
    build_failed_execution(
        chain_id,
        block,
        gas_used,
        gas_limit,
        Bytes::new(),
        build_halt_failure(reason),
        fee_settlement,
    )
}

fn build_failed_execution(
    chain_id: u64,
    block: &Sealed<Header>,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
    fee_settlement: &EvmFeeSettlement,
) -> EvmExecution {
    EvmExecution {
        chain_id,
        context: simulated_block(block),
        gas_limit,
        outcome: EvmOutcome::Failed {
            details: EvmExecutionDetails {
                gas_used,
                gas_charged: gas_used,
                fee: fee_settlement.fee,
                burnt_fee: fee_settlement.burnt_fee,
                output,
            },
            failure,
        },
    }
}

fn simulated_block(block: &Sealed<Header>) -> EvmBlockContext {
    EvmBlockContext {
        number: block.number(),
        hash: block.hash(),
    }
}

fn build_invalid_transaction_failure(error: InvalidTransaction) -> EvmExecutionFailure {
    let code = match error {
        InvalidTransaction::NonceTooLow { .. } => EvmExecutionFailureCode::NonceTooLow,
        InvalidTransaction::NonceTooHigh { .. } => EvmExecutionFailureCode::NonceTooHigh,
        InvalidTransaction::NonceOverflowInTransaction => EvmExecutionFailureCode::NonceOverflow,
        InvalidTransaction::LackOfFundForMaxFee { .. } => {
            EvmExecutionFailureCode::InsufficientFunds
        }
        InvalidTransaction::PriorityFeeGreaterThanMaxFee => {
            EvmExecutionFailureCode::PriorityFeeGreaterThanMaxFee
        }
        InvalidTransaction::GasPriceLessThanBasefee => {
            EvmExecutionFailureCode::GasPriceLessThanBaseFee
        }
        InvalidTransaction::CallerGasLimitMoreThanBlock
        | InvalidTransaction::TxGasLimitGreaterThanCap { .. } => {
            EvmExecutionFailureCode::GasLimitExceedsBlockGasLimit
        }
        InvalidTransaction::CallGasCostMoreThanGasLimit { .. }
        | InvalidTransaction::GasFloorMoreThanGasLimit { .. } => {
            EvmExecutionFailureCode::IntrinsicGasTooLow
        }
        InvalidTransaction::RejectCallerWithCode => EvmExecutionFailureCode::SenderHasCode,
        InvalidTransaction::InvalidChainId | InvalidTransaction::MissingChainId => {
            EvmExecutionFailureCode::InvalidChainId
        }
        InvalidTransaction::AccessListNotSupported
        | InvalidTransaction::Eip2930NotSupported
        | InvalidTransaction::Eip1559NotSupported
        | InvalidTransaction::Eip4844NotSupported
        | InvalidTransaction::Eip7702NotSupported
        | InvalidTransaction::Eip7873NotSupported => {
            EvmExecutionFailureCode::TransactionTypeNotSupported
        }
        InvalidTransaction::OverflowPaymentInTransaction
        | InvalidTransaction::CreateInitCodeSizeLimit
        | InvalidTransaction::MaxFeePerBlobGasNotSupported
        | InvalidTransaction::BlobVersionedHashesNotSupported
        | InvalidTransaction::BlobGasPriceGreaterThanMax { .. }
        | InvalidTransaction::EmptyBlobs
        | InvalidTransaction::BlobCreateTransaction
        | InvalidTransaction::TooManyBlobs { .. }
        | InvalidTransaction::BlobVersionNotSupported
        | InvalidTransaction::AuthorizationListNotSupported
        | InvalidTransaction::AuthorizationListInvalidFields
        | InvalidTransaction::EmptyAuthorizationList
        | InvalidTransaction::Eip7873MissingTarget
        | InvalidTransaction::Str(_) => EvmExecutionFailureCode::InvalidTransaction,
    };

    EvmExecutionFailure {
        code,
        message: error.to_string(),
        reason: None,
    }
}

fn build_revert_failure(output: &Bytes) -> EvmExecutionFailure {
    EvmExecutionFailure {
        code: EvmExecutionFailureCode::Revert,
        message: "execution reverted".to_string(),
        reason: decode_revert_reason(output),
    }
}

fn decode_revert_reason(output: &Bytes) -> Option<String> {
    Revert::abi_decode(output.as_ref())
        .map(|revert| revert.reason().to_string())
        .or_else(|_| {
            Panic::abi_decode(output.as_ref()).map(|panic| panic.as_geth_str().into_owned())
        })
        .ok()
}

fn build_halt_failure(reason: HaltReason) -> EvmExecutionFailure {
    let code = match reason {
        HaltReason::OutOfGas(_) => EvmExecutionFailureCode::OutOfGas,
        HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode => {
            EvmExecutionFailureCode::InvalidOpcode
        }
        HaltReason::InvalidJump => EvmExecutionFailureCode::InvalidJump,
        HaltReason::StackUnderflow => EvmExecutionFailureCode::StackUnderflow,
        HaltReason::StackOverflow => EvmExecutionFailureCode::StackOverflow,
        HaltReason::NonceOverflow => EvmExecutionFailureCode::NonceOverflow,
        _ => EvmExecutionFailureCode::ExecutionFailed,
    };

    EvmExecutionFailure {
        code,
        message: reason.to_string(),
        reason: None,
    }
}

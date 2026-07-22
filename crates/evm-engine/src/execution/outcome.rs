use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::Bytes;
use revm::context_interface::result::{ExecutionResult, HaltReason, InvalidTransaction};

use crate::{
    EvmExecution, EvmExecutionFailure, EvmExecutionFailureCode, EvmExecutionOutcome,
    EvmTransaction, SimulatedBlock,
};

use super::fee_settlement::TransactionFeeSettlement;
use crate::ResolvedBlock;

pub(super) fn build_execution(
    result: ExecutionResult<HaltReason>,
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    fee_settlement: &TransactionFeeSettlement,
) -> EvmExecution {
    match result {
        ExecutionResult::Success { gas, output, .. } => EvmExecution {
            chain_id,
            block: simulated_block(resolved_block),
            gas_limit: gas.limit(),
            outcome: EvmExecutionOutcome::Success {
                gas_used: gas.used(),
                fee: fee_settlement.fee,
                burnt_fee: fee_settlement.burnt_fee,
                output: output.into_data(),
            },
        },
        ExecutionResult::Revert { gas, output, .. } => build_revert_execution(
            chain_id,
            resolved_block,
            gas.used(),
            gas.limit(),
            output,
            fee_settlement,
        ),
        ExecutionResult::Halt { reason, gas, .. } => build_halt_execution(
            chain_id,
            resolved_block,
            gas.used(),
            gas.limit(),
            reason,
            fee_settlement,
        ),
    }
}

pub(super) fn build_not_executed(
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    transaction: &EvmTransaction,
    error: InvalidTransaction,
) -> EvmExecution {
    EvmExecution {
        chain_id,
        block: simulated_block(resolved_block),
        gas_limit: transaction.gas_limit,
        outcome: EvmExecutionOutcome::NotExecuted {
            failure: build_invalid_transaction_failure(error),
        },
    }
}

fn build_revert_execution(
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    fee_settlement: &TransactionFeeSettlement,
) -> EvmExecution {
    let failure = build_revert_failure(&output);

    build_failed_execution(
        chain_id,
        resolved_block,
        gas_used,
        gas_limit,
        output,
        failure,
        fee_settlement,
    )
}

fn build_halt_execution(
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    gas_used: u64,
    gas_limit: u64,
    reason: HaltReason,
    fee_settlement: &TransactionFeeSettlement,
) -> EvmExecution {
    build_failed_execution(
        chain_id,
        resolved_block,
        gas_used,
        gas_limit,
        Bytes::new(),
        build_halt_failure(reason),
        fee_settlement,
    )
}

fn build_failed_execution(
    chain_id: u64,
    resolved_block: &ResolvedBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
    fee_settlement: &TransactionFeeSettlement,
) -> EvmExecution {
    EvmExecution {
        chain_id,
        block: simulated_block(resolved_block),
        gas_limit,
        outcome: EvmExecutionOutcome::Failed {
            gas_used,
            fee: fee_settlement.fee,
            burnt_fee: fee_settlement.burnt_fee,
            output,
            failure,
        },
    }
}

fn simulated_block(resolved_block: &ResolvedBlock) -> SimulatedBlock {
    SimulatedBlock {
        number: resolved_block.number(),
        hash: resolved_block.hash(),
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

#[cfg(test)]
mod tests {
    use alloy::sol_types::{Panic, Revert, SolError};
    use alloy_primitives::Bytes;

    use super::{EvmExecutionFailureCode, build_revert_failure, decode_revert_reason};

    #[test]
    fn decode_revert_reason_extracts_standard_error_string() {
        let output = Bytes::from(Revert::from("dry run reverted").abi_encode());

        assert_eq!(
            decode_revert_reason(&output),
            Some("dry run reverted".to_string())
        );
    }

    #[test]
    fn decode_revert_reason_extracts_solidity_panic() {
        let output = Bytes::from(Panic::from(0x11_u64).abi_encode());

        assert_eq!(
            decode_revert_reason(&output),
            Some("arithmetic underflow or overflow".to_string())
        );
    }

    #[test]
    fn build_revert_failure_keeps_reason_empty_for_unknown_payload() {
        let failure = build_revert_failure(&Bytes::from_static(b"\x12\x34\x56\x78"));

        assert_eq!(failure.code, EvmExecutionFailureCode::Revert);
        assert_eq!(failure.reason, None);
    }
}

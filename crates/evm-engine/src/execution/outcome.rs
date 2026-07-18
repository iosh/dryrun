use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::{Bytes, U256};
use revm::context_interface::result::{ExecutionResult, HaltReason, InvalidTransaction};

use crate::{
    EvmExecution, EvmExecutionFailure, EvmExecutionFailureCode, EvmExecutionStatus, EvmSimulation,
    EvmTransaction, SimulatedBlock, change_observation::Observation,
    transaction_changes::ChangeCandidate,
};

use super::{
    ExecutionArtifacts, MainnetAlloyEvm, change_extraction::build_changes,
    fee_settlement::TransactionFeeSettlement, provider::ResolvedExecutionBlock,
};

pub(super) fn build_execution_artifacts(
    result: ExecutionResult<HaltReason>,
    observations: Vec<Observation>,
    resolved_block: &ResolvedExecutionBlock,
    fee_settlement: TransactionFeeSettlement,
) -> ExecutionArtifacts {
    match result {
        ExecutionResult::Success { gas, output, .. } => ExecutionArtifacts {
            chain_id: resolved_block.chain_id,
            block: simulated_block(resolved_block),
            status: EvmExecutionStatus::Success,
            gas_used: gas.used(),
            gas_limit: gas.limit(),
            fee_settlement: Some(fee_settlement),
            output: output.into_data(),
            failure: None,
            observations,
        },
        ExecutionResult::Revert { gas, output, .. } => build_revert_artifacts(
            resolved_block,
            gas.used(),
            gas.limit(),
            output,
            fee_settlement,
        ),
        ExecutionResult::Halt { reason, gas, .. } => build_halt_artifacts(
            resolved_block,
            gas.used(),
            gas.limit(),
            reason,
            fee_settlement,
        ),
    }
}

pub(super) fn build_simulation<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    artifacts: ExecutionArtifacts,
    transaction: &EvmTransaction,
    candidates: Vec<ChangeCandidate>,
) -> EvmSimulation {
    let changes = build_changes(evm, &artifacts, transaction, candidates);

    let ExecutionArtifacts {
        chain_id,
        block,
        status,
        gas_used,
        gas_limit,
        fee_settlement,
        output,
        failure,
        ..
    } = artifacts;
    let (fee, burnt_fee) = fee_settlement
        .map(|settlement| (settlement.fee, settlement.burnt_fee))
        .unwrap_or((U256::ZERO, U256::ZERO));

    EvmSimulation::new(
        EvmExecution {
            chain_id,
            block,
            status,
            gas_used,
            gas_limit,
            fee,
            burnt_fee,
            output,
            failure,
        },
        changes,
    )
}

pub(super) fn build_invalid_transaction_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    transaction: &EvmTransaction,
    error: InvalidTransaction,
) -> ExecutionArtifacts {
    ExecutionArtifacts {
        chain_id: resolved_block.chain_id,
        block: simulated_block(resolved_block),
        status: EvmExecutionStatus::NotExecuted,
        gas_used: 0,
        gas_limit: transaction.gas_limit,
        fee_settlement: None,
        output: Bytes::new(),
        failure: Some(build_invalid_transaction_failure(error)),
        observations: Vec::new(),
    }
}

fn build_revert_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    fee_settlement: TransactionFeeSettlement,
) -> ExecutionArtifacts {
    let failure = build_revert_failure(&output);

    build_failed_artifacts(
        resolved_block,
        gas_used,
        gas_limit,
        output,
        failure,
        fee_settlement,
    )
}

fn build_halt_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    reason: HaltReason,
    fee_settlement: TransactionFeeSettlement,
) -> ExecutionArtifacts {
    build_failed_artifacts(
        resolved_block,
        gas_used,
        gas_limit,
        Bytes::new(),
        build_halt_failure(reason),
        fee_settlement,
    )
}

fn build_failed_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
    fee_settlement: TransactionFeeSettlement,
) -> ExecutionArtifacts {
    ExecutionArtifacts {
        chain_id: resolved_block.chain_id,
        block: simulated_block(resolved_block),
        status: EvmExecutionStatus::Failed,
        gas_used,
        gas_limit,
        fee_settlement: Some(fee_settlement),
        output,
        failure: Some(failure),
        observations: Vec::new(),
    }
}

fn simulated_block(resolved_block: &ResolvedExecutionBlock) -> SimulatedBlock {
    SimulatedBlock {
        number: resolved_block.block.number(),
        hash: resolved_block.block.hash(),
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

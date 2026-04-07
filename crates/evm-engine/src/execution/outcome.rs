use alloy::sol_types::{Panic, Revert, SolError};
use alloy_primitives::Bytes;
use revm::context_interface::result::{ExecutionResult, HaltReason, InvalidTransaction};

use crate::{
    EvmExecution, EvmExecutionFailure, EvmExecutionStatus, EvmSimulation, EvmTransaction,
    SimulatedBlock, change_observation::Observation,
};

use super::{
    ExecutionArtifacts, MainnetAlloyEvm, change_extraction::extract_changes,
    provider::ResolvedExecutionBlock,
};

pub(super) fn build_execution_artifacts(
    result: ExecutionResult<HaltReason>,
    observations: Vec<Observation>,
    resolved_block: &ResolvedExecutionBlock,
) -> ExecutionArtifacts {
    match result {
        ExecutionResult::Success { gas, output, .. } => ExecutionArtifacts {
            chain_id: resolved_block.chain_id,
            block: simulated_block(resolved_block),
            status: EvmExecutionStatus::Success,
            gas_used: gas.used(),
            gas_limit: gas.limit(),
            output: output.into_data(),
            failure: None,
            observations,
        },
        ExecutionResult::Revert { gas, output, .. } => {
            build_revert_artifacts(resolved_block, gas.used(), gas.limit(), output)
        }
        ExecutionResult::Halt { reason, gas, .. } => {
            build_halt_artifacts(resolved_block, gas.used(), gas.limit(), reason)
        }
    }
}

pub(super) fn build_simulation<INSP>(
    evm: &mut MainnetAlloyEvm<INSP>,
    artifacts: ExecutionArtifacts,
    transaction: &EvmTransaction,
) -> EvmSimulation {
    // Changes are derived after execution so detectors can work from the final,
    // revert-filtered observation stream.
    let changes = extract_changes(evm, &artifacts, transaction);

    let ExecutionArtifacts {
        chain_id,
        block,
        status,
        gas_used,
        gas_limit,
        output,
        failure,
        ..
    } = artifacts;

    EvmSimulation::new(
        EvmExecution {
            chain_id,
            block,
            status,
            gas_used,
            gas_limit,
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
    build_failed_artifacts(
        resolved_block,
        0,
        transaction.gas_limit,
        Bytes::new(),
        build_invalid_transaction_failure(error),
    )
}

fn build_revert_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
) -> ExecutionArtifacts {
    let failure = build_revert_failure(&output);

    build_failed_artifacts(resolved_block, gas_used, gas_limit, output, failure)
}

fn build_halt_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    reason: HaltReason,
) -> ExecutionArtifacts {
    build_failed_artifacts(
        resolved_block,
        gas_used,
        gas_limit,
        Bytes::new(),
        build_halt_failure(reason),
    )
}

fn build_failed_artifacts(
    resolved_block: &ResolvedExecutionBlock,
    gas_used: u64,
    gas_limit: u64,
    output: Bytes,
    failure: EvmExecutionFailure,
) -> ExecutionArtifacts {
    ExecutionArtifacts {
        chain_id: resolved_block.chain_id,
        block: simulated_block(resolved_block),
        status: EvmExecutionStatus::Failed,
        gas_used,
        gas_limit,
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
        InvalidTransaction::NonceTooLow { .. } => "NONCE_TOO_LOW",
        InvalidTransaction::NonceTooHigh { .. } => "NONCE_TOO_HIGH",
        InvalidTransaction::LackOfFundForMaxFee { .. } => "INSUFFICIENT_FUNDS",
        InvalidTransaction::GasPriceLessThanBasefee => "GAS_PRICE_LESS_THAN_BASE_FEE",
        InvalidTransaction::CallerGasLimitMoreThanBlock => "GAS_LIMIT_EXCEEDS_BLOCK_GAS_LIMIT",
        InvalidTransaction::Eip2930NotSupported => "EIP2930_NOT_SUPPORTED",
        InvalidTransaction::Eip1559NotSupported => "EIP1559_NOT_SUPPORTED",
        _ => "INVALID_TRANSACTION",
    };

    EvmExecutionFailure {
        code: code.to_string(),
        message: error.to_string(),
        reason: None,
    }
}

fn build_revert_failure(output: &Bytes) -> EvmExecutionFailure {
    EvmExecutionFailure {
        code: "REVERT".to_string(),
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
        HaltReason::OutOfGas(_) => "OUT_OF_GAS",
        HaltReason::OpcodeNotFound | HaltReason::InvalidFEOpcode => "INVALID_OPCODE",
        HaltReason::InvalidJump => "INVALID_JUMP",
        HaltReason::StackUnderflow => "STACK_UNDERFLOW",
        HaltReason::StackOverflow => "STACK_OVERFLOW",
        HaltReason::OutOfOffset => "OUT_OF_OFFSET",
        HaltReason::CreateCollision => "CREATE_COLLISION",
        HaltReason::NotActivated => "NOT_ACTIVATED",
        HaltReason::PrecompileError | HaltReason::PrecompileErrorWithContext(_) => {
            "PRECOMPILE_ERROR"
        }
        HaltReason::NonceOverflow => "NONCE_OVERFLOW",
        HaltReason::CreateContractSizeLimit => "CREATE_CONTRACT_SIZE_LIMIT",
        HaltReason::CreateContractStartingWithEF => "CREATE_CONTRACT_STARTING_WITH_EF",
        HaltReason::CreateInitCodeSizeLimit => "CREATE_INITCODE_SIZE_LIMIT",
        HaltReason::OverflowPayment => "OVERFLOW_PAYMENT",
        HaltReason::StateChangeDuringStaticCall => "STATE_CHANGE_DURING_STATIC_CALL",
        HaltReason::CallNotAllowedInsideStatic => "CALL_NOT_ALLOWED_INSIDE_STATIC",
        HaltReason::OutOfFunds => "OUT_OF_FUNDS",
        HaltReason::CallTooDeep => "CALL_TOO_DEEP",
    };

    EvmExecutionFailure {
        code: code.to_string(),
        message: reason.to_string(),
        reason: None,
    }
}

#[cfg(test)]
mod tests {
    use alloy::sol_types::{Panic, Revert, SolError};
    use alloy_primitives::Bytes;

    use super::{build_revert_failure, decode_revert_reason};

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

        assert_eq!(failure.code, "REVERT");
        assert_eq!(failure.reason, None);
    }
}

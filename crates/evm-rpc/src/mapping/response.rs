use alloy_primitives::{Bytes, U256};
use evm_simulation::{
    EvmAccountDelegationChange, EvmBlockContext, EvmChanges, EvmExecutionOutcome, EvmHaltReason,
    EvmNativeBalanceChange, EvmNativeCurrency, EvmSimulation, EvmStateChange, EvmSuccessOutput,
    EvmTransactionRejection,
};

use crate::interface as rpc;

impl From<EvmSimulation> for rpc::EvmSimulateTransactionResponse {
    fn from(output: EvmSimulation) -> Self {
        let EvmSimulation {
            context: block,
            transaction,
            execution,
            changes,
        } = output;
        let chain_id = transaction.chain_id;
        let gas_limit = transaction.gas_limit;

        let (status, result, output, failure) = match execution {
            EvmExecutionOutcome::Success { result, output, .. } => {
                let output = match output {
                    EvmSuccessOutput::Call { return_data } => return_data,
                    EvmSuccessOutput::Create { runtime_code, .. } => runtime_code,
                };
                (rpc::ExecutionStatus::Success, Some(result), output, None)
            }
            EvmExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            } => (
                rpc::ExecutionStatus::Failed,
                Some(result),
                revert_data,
                Some(rpc::ExecutionFailure {
                    code: "REVERT".to_string(),
                    message: "execution reverted".to_string(),
                    reason: reason.map(|reason| reason.to_string()),
                }),
            ),
            EvmExecutionOutcome::Halted { result, reason } => (
                rpc::ExecutionStatus::Failed,
                Some(result),
                Bytes::new(),
                Some(reason.into()),
            ),
            EvmExecutionOutcome::NotExecuted(rejection) => (
                rpc::ExecutionStatus::NotExecuted,
                None,
                Bytes::new(),
                Some(rejection.into()),
            ),
        };
        let (gas_used, fee, burnt_fee) = result.map_or((0, U256::ZERO, U256::ZERO), |result| {
            let protocol_fee = result.fee();
            (
                result.gas().gas_used(),
                protocol_fee.total_charged_amount(),
                protocol_fee.total_burnt_amount(),
            )
        });

        Self {
            execution: rpc::Execution {
                chain_id,
                block: block.into(),
                status,
                gas_used,
                gas_limit,
                fee,
                burnt_fee,
                output,
                failure,
            },
            changes: changes.into(),
        }
    }
}

impl From<EvmChanges> for rpc::Changes {
    fn from(changes: EvmChanges) -> Self {
        match changes {
            EvmChanges::Complete(changes) => Self::Complete {
                items: changes.items().iter().cloned().map(Into::into).collect(),
            },
            EvmChanges::Unavailable(error) => Self::Unavailable {
                error: error.to_string(),
            },
        }
    }
}

impl From<EvmStateChange> for rpc::StateChange {
    fn from(change: EvmStateChange) -> Self {
        match change {
            EvmStateChange::NativeBalance(change) => change.into(),
            EvmStateChange::AccountDelegation(change) => change.into(),
        }
    }
}

impl From<EvmNativeBalanceChange> for rpc::StateChange {
    fn from(change: EvmNativeBalanceChange) -> Self {
        Self::NativeBalance {
            account: change.account,
            before: change.before,
            after: change.after,
            currency: change.currency.into(),
        }
    }
}

impl From<EvmAccountDelegationChange> for rpc::StateChange {
    fn from(change: EvmAccountDelegationChange) -> Self {
        Self::AccountDelegation {
            account: change.account,
            before: change.before.into(),
            after: change.after.into(),
        }
    }
}

impl From<evm_simulation::EvmAccountDelegation> for rpc::DelegationState {
    fn from(state: evm_simulation::EvmAccountDelegation) -> Self {
        Self {
            delegate: state.delegate,
            nonce: state.nonce,
        }
    }
}

impl From<EvmBlockContext> for rpc::EvmBlockContext {
    fn from(block: EvmBlockContext) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<EvmHaltReason> for rpc::ExecutionFailure {
    fn from(reason: EvmHaltReason) -> Self {
        Self {
            code: halt_failure_code(&reason).to_string(),
            message: reason.to_string(),
            reason: None,
        }
    }
}

impl From<EvmTransactionRejection> for rpc::ExecutionFailure {
    fn from(rejection: EvmTransactionRejection) -> Self {
        Self {
            code: rejection_failure_code(&rejection).to_string(),
            message: rejection.to_string(),
            reason: None,
        }
    }
}

fn halt_failure_code(reason: &EvmHaltReason) -> &'static str {
    match reason {
        EvmHaltReason::OutOfGas(_) => "OUT_OF_GAS",
        EvmHaltReason::OpcodeNotFound | EvmHaltReason::InvalidFeOpcode => "INVALID_OPCODE",
        EvmHaltReason::InvalidJump => "INVALID_JUMP",
        EvmHaltReason::StackUnderflow => "STACK_UNDERFLOW",
        EvmHaltReason::StackOverflow => "STACK_OVERFLOW",
        EvmHaltReason::NonceOverflow => "NONCE_OVERFLOW",
        _ => "EXECUTION_FAILED",
    }
}

fn rejection_failure_code(rejection: &EvmTransactionRejection) -> &'static str {
    match rejection {
        EvmTransactionRejection::PriorityFeeGreaterThanMaxFee { .. } => {
            "PRIORITY_FEE_GREATER_THAN_MAX_FEE"
        }
        EvmTransactionRejection::GasPriceBelowBaseFee { .. } => "GAS_PRICE_LESS_THAN_BASE_FEE",
        EvmTransactionRejection::GasLimitExceedsBlockGasLimit { .. }
        | EvmTransactionRejection::GasLimitExceedsCap { .. } => "GAS_LIMIT_EXCEEDS_BLOCK_GAS_LIMIT",
        EvmTransactionRejection::IntrinsicGasExceedsGasLimit { .. }
        | EvmTransactionRejection::FloorGasExceedsGasLimit { .. } => "INTRINSIC_GAS_TOO_LOW",
        EvmTransactionRejection::SenderHasCode { .. } => "SENDER_HAS_CODE",
        EvmTransactionRejection::InsufficientFunds { .. } => "INSUFFICIENT_FUNDS",
        EvmTransactionRejection::NonceOverflow => "NONCE_OVERFLOW",
        EvmTransactionRejection::NonceTooHigh { .. } => "NONCE_TOO_HIGH",
        EvmTransactionRejection::NonceTooLow { .. } => "NONCE_TOO_LOW",
        EvmTransactionRejection::InvalidChainId { .. } => "INVALID_CHAIN_ID",
        EvmTransactionRejection::BlobGasPriceExceedsMaxFee { .. } => {
            "BLOB_GAS_PRICE_EXCEEDS_MAX_FEE"
        }
        EvmTransactionRejection::BlobCountExceedsLimit { .. } => "TOO_MANY_BLOBS",
        EvmTransactionRejection::UnsupportedBlobVersion { .. } => "UNSUPPORTED_BLOB_VERSION",
        EvmTransactionRejection::Eip2930NotActivated
        | EvmTransactionRejection::Eip1559NotActivated
        | EvmTransactionRejection::Eip4844NotActivated
        | EvmTransactionRejection::Eip7702NotActivated => "TRANSACTION_TYPE_NOT_SUPPORTED",
        _ => "INVALID_TRANSACTION",
    }
}

impl From<EvmNativeCurrency> for rpc::NativeCurrency {
    fn from(currency: EvmNativeCurrency) -> Self {
        Self {
            name: currency.name,
            symbol: currency.symbol,
            decimals: currency.decimals,
        }
    }
}

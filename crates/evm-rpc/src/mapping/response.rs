use alloy_primitives::{Address, Bytes, U256};
use evm_simulation::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, EvmBlockContext, EvmChange,
    EvmExecutionOutcome, EvmHaltReason, EvmSimulation, EvmSuccessOutput, EvmTransactionRejection,
    NativeCurrency, StandardChange,
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
            changes: changes.into_iter().map(Into::into).collect(),
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

impl From<NativeCurrency> for rpc::NativeCurrency {
    fn from(currency: NativeCurrency) -> Self {
        Self {
            name: currency.name,
            symbol: currency.symbol,
            decimals: currency.decimals,
        }
    }
}

impl From<Erc20Metadata> for rpc::Erc20Metadata {
    fn from(metadata: Erc20Metadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<Erc721CollectionMetadata> for rpc::Erc721CollectionMetadata {
    fn from(metadata: Erc721CollectionMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
        }
    }
}

impl From<EvmChange> for rpc::Change {
    fn from(change: EvmChange) -> Self {
        match change {
            EvmChange::NativeTransfer {
                from,
                to,
                raw_amount,
                currency,
            } => Self::NativeTransfer {
                from,
                to,
                raw_amount,
                currency: currency.into(),
            },
            EvmChange::SelfDestructBurn {
                contract_address,
                raw_amount,
                currency,
            } => Self::SelfDestructBurn {
                contract_address,
                raw_amount,
                currency: currency.into(),
            },
            EvmChange::WrappedNativeDeposit {
                contract_address,
                account,
                raw_amount,
                metadata,
            } => Self::WrappedNativeDeposit {
                contract_address,
                account,
                raw_amount,
                metadata: metadata.into(),
            },
            EvmChange::WrappedNativeWithdrawal {
                contract_address,
                account,
                raw_amount,
                metadata,
            } => Self::WrappedNativeWithdrawal {
                contract_address,
                account,
                raw_amount,
                metadata: metadata.into(),
            },
            EvmChange::Standard(change) => change.into(),
        }
    }
}

impl From<StandardChange<Address>> for rpc::Change {
    fn from(change: StandardChange<Address>) -> Self {
        match change {
            StandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            StandardChange::Erc20Approval {
                contract_address,
                owner,
                spender,
                approved_amount,
                metadata,
            } => Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                approved_amount,
                metadata: metadata.into(),
            },
            StandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata: metadata.into(),
            },
            StandardChange::Erc721Approval {
                contract_address,
                owner,
                approved_address,
                token_id,
                metadata,
            } => Self::Erc721Approval {
                contract_address,
                owner,
                approved_address,
                token_id,
                metadata: metadata.into(),
            },
            StandardChange::OperatorApproval {
                contract_address,
                owner,
                operator,
                approved,
            } => Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                approved,
            },
            StandardChange::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            },
            StandardChange::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<Erc1155TransferItem> for rpc::Erc1155TransferItem {
    fn from(item: Erc1155TransferItem) -> Self {
        Self {
            token_id: item.token_id,
            raw_amount: item.raw_amount,
        }
    }
}

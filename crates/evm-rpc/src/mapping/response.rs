use alloy_primitives::{Bytes, U256};
use evm_simulation::{
    Change, Erc20Metadata, Erc721CollectionMetadata, EvmBlockContext, EvmExecutionOutcome,
    EvmHaltReason, EvmSimulation, EvmSuccessOutput, EvmTransactionRejection, NativeMetadata,
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

impl From<NativeMetadata> for rpc::NativeMetadata {
    fn from(metadata: NativeMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
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

impl From<Change> for rpc::Change {
    fn from(change: Change) -> Self {
        match change {
            Change::NativeTransfer {
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Transfer {
                asset: rpc::TransferAsset::Native {
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            Change::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Transfer {
                asset: rpc::TransferAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            Change::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata,
            } => Self::Mint {
                asset: rpc::TokenMovementAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                to,
            },
            Change::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata,
            } => Self::Burn {
                asset: rpc::BurnAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
            },
            Change::SelfDestructBurn {
                contract_address,
                raw_amount,
                metadata,
            } => Self::Burn {
                asset: rpc::BurnAsset::Native {
                    raw_amount,
                    metadata: metadata.into(),
                },
                from: contract_address,
            },
            Change::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Transfer {
                asset: rpc::TransferAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            Change::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata,
            } => Self::Mint {
                asset: rpc::TokenMovementAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                to,
            },
            Change::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata,
            } => Self::Burn {
                asset: rpc::BurnAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                from,
            },
            Change::Erc1155Transfer {
                contract_address,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Transfer {
                asset: rpc::TransferAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                from,
                to,
            },
            Change::Erc1155Mint {
                contract_address,
                to,
                token_id,
                raw_amount,
            } => Self::Mint {
                asset: rpc::TokenMovementAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                to,
            },
            Change::Erc1155Burn {
                contract_address,
                from,
                token_id,
                raw_amount,
            } => Self::Burn {
                asset: rpc::BurnAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                from,
            },
            Change::Erc20Allowance {
                contract_address,
                owner,
                spender,
                raw_amount_before,
                raw_amount_after,
                metadata,
            } => Self::Allowance {
                asset: rpc::AllowanceAsset::Erc20 {
                    contract_address,
                    raw_amount_before,
                    raw_amount_after,
                    metadata: metadata.into(),
                },
                owner,
                spender,
            },
            Change::Erc721TokenApproval {
                contract_address,
                token_id,
                approved_address_before,
                approved_address_after,
                metadata,
            } => Self::TokenApproval {
                asset: rpc::TokenApprovalAsset::Erc721 {
                    contract_address,
                    token_id,
                    approved_address_before,
                    approved_address_after,
                    metadata: metadata.into(),
                },
            },
            Change::Erc721OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
                metadata,
            } => Self::OperatorApproval {
                asset: rpc::OperatorApprovalAsset::Erc721 {
                    contract_address,
                    metadata: metadata.into(),
                },
                owner,
                operator,
                approved_before,
                approved_after,
            },
            Change::Erc1155OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
            } => Self::OperatorApproval {
                asset: rpc::OperatorApprovalAsset::Erc1155 { contract_address },
                owner,
                operator,
                approved_before,
                approved_after,
            },
        }
    }
}

use alloy::primitives::{Bytes, U256};

use crate::interface as rpc;

impl From<evm_service::SimulateEvmTransactionOutput> for rpc::EvmSimulateTransactionResponse {
    fn from(output: evm_service::SimulateEvmTransactionOutput) -> Self {
        let (execution, changes) = output.into_parts();
        let evm_service::SimulationExecution {
            chain_id,
            context: block,
            gas_limit,
            outcome,
        } = execution;

        let (status, gas_used, fee, burnt_fee, output, failure) = match outcome {
            evm_service::ExecutionOutcome::Success(evm_service::ExecutedDetails {
                gas_used,
                gas_charged: _,
                fee,
                burnt_fee,
                output,
            }) => (
                rpc::ExecutionStatus::Success,
                gas_used,
                fee,
                burnt_fee,
                output,
                None,
            ),
            evm_service::ExecutionOutcome::Failed {
                details:
                    evm_service::ExecutedDetails {
                        gas_used,
                        gas_charged: _,
                        fee,
                        burnt_fee,
                        output,
                    },
                failure,
            } => (
                rpc::ExecutionStatus::Failed,
                gas_used,
                fee,
                burnt_fee,
                output,
                Some(failure.into()),
            ),
            evm_service::ExecutionOutcome::NotExecuted(failure) => (
                rpc::ExecutionStatus::NotExecuted,
                0,
                U256::ZERO,
                U256::ZERO,
                Bytes::new(),
                Some(failure.into()),
            ),
        };

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

impl From<evm_service::SimulatedBlock> for rpc::SimulatedBlock {
    fn from(block: evm_service::SimulatedBlock) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<evm_service::ExecutionFailure> for rpc::ExecutionFailure {
    fn from(failure: evm_service::ExecutionFailure) -> Self {
        Self {
            code: failure.code.as_str().to_string(),
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<evm_service::NativeMetadata> for rpc::NativeMetadata {
    fn from(metadata: evm_service::NativeMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<evm_service::Erc20Metadata> for rpc::Erc20Metadata {
    fn from(metadata: evm_service::Erc20Metadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<evm_service::Erc721CollectionMetadata> for rpc::Erc721CollectionMetadata {
    fn from(metadata: evm_service::Erc721CollectionMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
        }
    }
}

impl From<evm_service::Change> for rpc::Change {
    fn from(change: evm_service::Change) -> Self {
        match change {
            evm_service::Change::NativeTransfer {
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
            evm_service::Change::Erc20Transfer {
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
            evm_service::Change::Erc20Mint {
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
            evm_service::Change::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata,
            } => Self::Burn {
                asset: rpc::TokenMovementAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
            },
            evm_service::Change::Erc721Transfer {
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
            evm_service::Change::Erc721Mint {
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
            evm_service::Change::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata,
            } => Self::Burn {
                asset: rpc::TokenMovementAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                from,
            },
            evm_service::Change::Erc1155Transfer {
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
            evm_service::Change::Erc1155Mint {
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
            evm_service::Change::Erc1155Burn {
                contract_address,
                from,
                token_id,
                raw_amount,
            } => Self::Burn {
                asset: rpc::TokenMovementAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                from,
            },
            evm_service::Change::Erc20Allowance {
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
            evm_service::Change::Erc721TokenApproval {
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
            evm_service::Change::Erc721OperatorApproval {
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
            evm_service::Change::Erc1155OperatorApproval {
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

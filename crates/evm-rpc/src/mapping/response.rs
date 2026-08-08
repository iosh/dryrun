use alloy_primitives::{Bytes, U256};
use evm_simulation::{
    Change, Erc20Metadata, Erc721CollectionMetadata, EvmBlockContext, EvmExecution,
    EvmExecutionDetails, EvmExecutionFailure, EvmOutcome, EvmSimulation, NativeMetadata,
};

use crate::interface as rpc;

impl From<EvmSimulation> for rpc::EvmSimulateTransactionResponse {
    fn from(output: EvmSimulation) -> Self {
        let (execution, changes) = output.into_parts();
        let EvmExecution {
            chain_id,
            context: block,
            gas_limit,
            outcome,
        } = execution;

        let (status, gas_used, fee, burnt_fee, output, failure) = match outcome {
            EvmOutcome::Success(EvmExecutionDetails {
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
            EvmOutcome::Failed {
                details:
                    EvmExecutionDetails {
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
            EvmOutcome::NotExecuted(failure) => (
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

impl From<EvmBlockContext> for rpc::EvmBlockContext {
    fn from(block: EvmBlockContext) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<EvmExecutionFailure> for rpc::ExecutionFailure {
    fn from(failure: EvmExecutionFailure) -> Self {
        Self {
            code: failure.code.as_str().to_string(),
            message: failure.message,
            reason: failure.reason,
        }
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
                asset: rpc::TokenMovementAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
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
                asset: rpc::TokenMovementAsset::Erc721 {
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
                asset: rpc::TokenMovementAsset::Erc1155 {
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

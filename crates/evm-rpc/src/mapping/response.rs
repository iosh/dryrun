use alloy::primitives::{Bytes, U256};

use crate::interface as rpc;

impl From<evm_service::SimulateEvmTransactionOutput> for rpc::EvmSimulateTransactionResponse {
    fn from(output: evm_service::SimulateEvmTransactionOutput) -> Self {
        let (execution, changes) = output.into_parts();
        let evm_service::SimulationExecution {
            chain_id,
            block,
            gas_limit,
            outcome,
        } = execution;

        let (status, gas_used, fee, burnt_fee, output, failure) = match outcome {
            evm_service::ExecutionOutcome::Success {
                gas_used,
                fee,
                burnt_fee,
                output,
            } => (
                rpc::ExecutionStatus::Success,
                gas_used,
                fee,
                burnt_fee,
                output,
                None,
            ),
            evm_service::ExecutionOutcome::Failed {
                gas_used,
                fee,
                burnt_fee,
                output,
                failure,
            } => (
                rpc::ExecutionStatus::Failed,
                gas_used,
                fee,
                burnt_fee,
                output,
                Some(failure.into()),
            ),
            evm_service::ExecutionOutcome::NotExecuted { failure } => (
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

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, Bytes, U256};
    use serde_json::json;

    use crate::interface as rpc;

    fn address(byte: u8) -> Address {
        Address::repeat_byte(byte)
    }

    fn successful_execution() -> evm_service::SimulationExecution {
        evm_service::SimulationExecution {
            chain_id: 1,
            block: evm_service::SimulatedBlock {
                number: 0x1234,
                hash: B256::repeat_byte(0xaa),
            },
            gas_limit: 0x5300,
            outcome: evm_service::ExecutionOutcome::Success {
                gas_used: 0x5208,
                fee: U256::ZERO,
                burnt_fee: U256::ZERO,
                output: Bytes::new(),
            },
        }
    }

    #[test]
    fn movement_changes_serialize_as_flat_discriminated_variants() {
        let output = evm_service::SimulateEvmTransactionOutput {
            execution: successful_execution(),
            changes: vec![
                evm_service::Change::NativeTransfer {
                    from: address(0x01),
                    to: address(0x02),
                    raw_amount: U256::from(10_u64),
                    metadata: evm_service::NativeMetadata {
                        name: Some("Ether".to_string()),
                        symbol: Some("ETH".to_string()),
                        decimals: Some(18),
                    },
                },
                evm_service::Change::Erc20Mint {
                    contract_address: address(0x03),
                    to: address(0x04),
                    raw_amount: U256::from(20_u64),
                    metadata: evm_service::Erc20Metadata {
                        name: None,
                        symbol: Some("TOK".to_string()),
                        decimals: Some(6),
                    },
                },
                evm_service::Change::Erc721Burn {
                    contract_address: address(0x05),
                    from: address(0x06),
                    token_id: U256::from(42_u64),
                    metadata: evm_service::Erc721CollectionMetadata {
                        name: Some("Collection".to_string()),
                        symbol: None,
                    },
                },
                evm_service::Change::Erc1155Transfer {
                    contract_address: address(0x07),
                    from: address(0x08),
                    to: address(0x09),
                    token_id: U256::from(7_u64),
                    raw_amount: U256::from(3_u64),
                },
            ],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        let serialized = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(
            serialized["changes"][0],
            json!({
                "changeType": "TRANSFER",
                "assetType": "NATIVE",
                "from": address(0x01),
                "to": address(0x02),
                "rawAmount": "0xa",
                "name": "Ether",
                "symbol": "ETH",
                "decimals": 18
            })
        );
        assert_eq!(serialized["changes"][1]["changeType"], json!("MINT"));
        assert_eq!(serialized["changes"][1]["assetType"], json!("ERC20"));
        assert_eq!(serialized["changes"][1]["rawAmount"], json!("0x14"));
        assert!(serialized["changes"][1].get("from").is_none());
        assert!(serialized["changes"][1].get("name").is_none());
        assert_eq!(serialized["changes"][2]["changeType"], json!("BURN"));
        assert_eq!(serialized["changes"][2]["tokenId"], json!("0x2a"));
        assert!(serialized["changes"][2].get("rawAmount").is_none());
        assert!(serialized["changes"][2].get("to").is_none());
        assert_eq!(serialized["changes"][3]["assetType"], json!("ERC1155"));
        assert_eq!(serialized["changes"][3]["tokenId"], json!("0x7"));
        assert_eq!(serialized["changes"][3]["rawAmount"], json!("0x3"));
    }

    #[test]
    fn authorization_changes_serialize_before_and_after_state() {
        let output = evm_service::SimulateEvmTransactionOutput {
            execution: successful_execution(),
            changes: vec![
                evm_service::Change::Erc20Allowance {
                    contract_address: address(0x10),
                    owner: address(0x11),
                    spender: address(0x12),
                    raw_amount_before: U256::from(5_u64),
                    raw_amount_after: U256::from(9_u64),
                    metadata: evm_service::Erc20Metadata::default(),
                },
                evm_service::Change::Erc721TokenApproval {
                    contract_address: address(0x20),
                    token_id: U256::from(8_u64),
                    approved_address_before: Some(address(0x21)),
                    approved_address_after: None,
                    metadata: evm_service::Erc721CollectionMetadata::default(),
                },
                evm_service::Change::Erc721OperatorApproval {
                    contract_address: address(0x30),
                    owner: address(0x31),
                    operator: address(0x32),
                    approved_before: false,
                    approved_after: true,
                    metadata: evm_service::Erc721CollectionMetadata::default(),
                },
                evm_service::Change::Erc1155OperatorApproval {
                    contract_address: address(0x40),
                    owner: address(0x41),
                    operator: address(0x42),
                    approved_before: true,
                    approved_after: false,
                },
            ],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        let serialized = serde_json::to_value(response).expect("response should serialize");

        assert_eq!(serialized["changes"][0]["changeType"], json!("ALLOWANCE"));
        assert_eq!(serialized["changes"][0]["assetType"], json!("ERC20"));
        assert_eq!(serialized["changes"][0]["rawAmountBefore"], json!("0x5"));
        assert_eq!(serialized["changes"][0]["rawAmountAfter"], json!("0x9"));
        assert_eq!(
            serialized["changes"][1]["approvedAddressBefore"],
            json!(address(0x21))
        );
        assert!(serialized["changes"][1]["approvedAddressAfter"].is_null());
        assert_eq!(
            serialized["changes"][2]["changeType"],
            json!("OPERATOR_APPROVAL")
        );
        assert_eq!(serialized["changes"][2]["assetType"], json!("ERC721"));
        assert_eq!(serialized["changes"][2]["approvedBefore"], json!(false));
        assert_eq!(serialized["changes"][2]["approvedAfter"], json!(true));
        assert_eq!(serialized["changes"][3]["assetType"], json!("ERC1155"));
        assert_eq!(serialized["changes"][3]["approvedBefore"], json!(true));
        assert_eq!(serialized["changes"][3]["approvedAfter"], json!(false));
    }

    #[test]
    fn failed_service_output_maps_error_into_execution() {
        let output = evm_service::SimulateEvmTransactionOutput {
            execution: evm_service::SimulationExecution {
                outcome: evm_service::ExecutionOutcome::Failed {
                    gas_used: 0x5208,
                    fee: U256::ZERO,
                    burnt_fee: U256::ZERO,
                    output: Bytes::new(),
                    failure: evm_service::ExecutionFailure {
                        code: evm_service::EvmExecutionFailureCode::Revert,
                        message: "execution reverted".to_string(),
                        reason: Some("insufficient output".to_string()),
                    },
                },
                ..successful_execution()
            },
            changes: Vec::new(),
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.execution.status, rpc::ExecutionStatus::Failed);

        let failure = response
            .execution
            .failure
            .expect("expected execution failure");
        assert_eq!(failure.code, "REVERT");
        assert_eq!(failure.message, "execution reverted");
        assert_eq!(failure.reason.as_deref(), Some("insufficient output"));
    }

    #[test]
    fn not_executed_outcome_maps_wire_zero_values() {
        let output = evm_service::SimulateEvmTransactionOutput {
            execution: evm_service::SimulationExecution {
                outcome: evm_service::ExecutionOutcome::NotExecuted {
                    failure: evm_service::ExecutionFailure {
                        code: evm_service::EvmExecutionFailureCode::InsufficientFunds,
                        message: "insufficient funds".to_string(),
                        reason: None,
                    },
                },
                ..successful_execution()
            },
            changes: Vec::new(),
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();

        assert_eq!(response.execution.status, rpc::ExecutionStatus::NotExecuted);
        assert_eq!(response.execution.gas_used, 0);
        assert_eq!(response.execution.gas_limit, 0x5300);
        assert_eq!(response.execution.fee, U256::ZERO);
        assert_eq!(response.execution.burnt_fee, U256::ZERO);
        assert!(response.execution.output.is_empty());
        assert_eq!(
            response
                .execution
                .failure
                .expect("expected execution failure")
                .code,
            "INSUFFICIENT_FUNDS"
        );
    }
}

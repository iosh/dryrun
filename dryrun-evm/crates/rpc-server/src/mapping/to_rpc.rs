use crate::interface as rpc;

impl From<simulation_service::SimulateEvmTransactionOutput>
    for rpc::EvmSimulateTransactionResponse
{
    fn from(output: simulation_service::SimulateEvmTransactionOutput) -> Self {
        let simulation_service::SimulateEvmTransactionOutput { execution, changes } = output;

        Self {
            execution: rpc::Execution {
                chain_id: execution.chain_id,
                block: execution.block.into(),
                status: execution.status.into(),
                gas_used: execution.gas_used,
                gas_limit: execution.gas_limit,
                output: execution.output,
                error: execution.failure.map(Into::into),
            },
            changes: changes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<simulation_service::SimulatedBlock> for rpc::SimulatedBlock {
    fn from(block: simulation_service::SimulatedBlock) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<simulation_service::ExecutionStatus> for rpc::SimulationStatus {
    fn from(status: simulation_service::ExecutionStatus) -> Self {
        match status {
            simulation_service::ExecutionStatus::Success => Self::Success,
            simulation_service::ExecutionStatus::Failed => Self::Failed,
        }
    }
}

impl From<simulation_service::ExecutionFailure> for rpc::ExecutionError {
    fn from(error: simulation_service::ExecutionFailure) -> Self {
        Self {
            code: error.code,
            message: error.message,
            reason: error.reason,
        }
    }
}

impl From<simulation_service::NativeAssetDisplay> for rpc::NativeAssetDisplay {
    fn from(display: simulation_service::NativeAssetDisplay) -> Self {
        Self {
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<simulation_service::Erc20AssetDisplay> for rpc::Erc20AssetDisplay {
    fn from(display: simulation_service::Erc20AssetDisplay) -> Self {
        Self {
            name: display.name,
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<simulation_service::Erc721CollectionDisplay> for rpc::Erc721CollectionDisplay {
    fn from(collection: simulation_service::Erc721CollectionDisplay) -> Self {
        Self {
            name: collection.name,
            symbol: collection.symbol,
        }
    }
}

impl From<simulation_service::Erc1155CollectionDisplay> for rpc::Erc1155CollectionDisplay {
    fn from(collection: simulation_service::Erc1155CollectionDisplay) -> Self {
        Self {
            name: collection.name,
        }
    }
}

impl From<simulation_service::NftTokenDisplay> for rpc::NftTokenDisplay {
    fn from(token: simulation_service::NftTokenDisplay) -> Self {
        Self { name: token.name }
    }
}

impl From<simulation_service::Collection> for rpc::Collection {
    fn from(collection: simulation_service::Collection) -> Self {
        match collection {
            simulation_service::Collection::Erc721 {
                contract_address,
                collection,
            } => Self::Erc721 {
                contract_address,
                collection: collection.map(Into::into),
            },
            simulation_service::Collection::Erc1155 {
                contract_address,
                collection,
            } => Self::Erc1155 {
                contract_address,
                collection: collection.map(Into::into),
            },
        }
    }
}

impl From<simulation_service::Asset> for rpc::Asset {
    fn from(asset: simulation_service::Asset) -> Self {
        match asset {
            simulation_service::Asset::Native { display } => Self::Native {
                display: display.map(Into::into),
            },
            simulation_service::Asset::Erc20 {
                contract_address,
                display,
            } => Self::Erc20 {
                contract_address,
                display: display.map(Into::into),
            },
            simulation_service::Asset::Erc721 {
                contract_address,
                token_id,
                collection,
                token,
            } => Self::Erc721 {
                contract_address,
                token_id,
                collection: collection.map(Into::into),
                token: token.map(Into::into),
            },
            simulation_service::Asset::Erc1155 {
                contract_address,
                token_id,
                collection,
                token,
            } => Self::Erc1155 {
                contract_address,
                token_id,
                collection: collection.map(Into::into),
                token: token.map(Into::into),
            },
        }
    }
}

impl From<simulation_service::Change> for rpc::Change {
    fn from(change: simulation_service::Change) -> Self {
        match change {
            simulation_service::Change::Transfer(change) => Self::Transfer {
                asset: change.asset.into(),
                from: change.from,
                to: change.to,
                amount: change.amount,
            },
            simulation_service::Change::Mint(change) => Self::Mint {
                asset: change.asset.into(),
                to: change.to,
                amount: change.amount,
            },
            simulation_service::Change::Burn(change) => Self::Burn {
                asset: change.asset.into(),
                from: change.from,
                amount: change.amount,
            },
            simulation_service::Change::Approval(change) => Self::Approval {
                asset: change.asset.into(),
                owner: change.owner,
                spender: change.spender,
                amount: change.amount,
            },
            simulation_service::Change::ApprovalForAll(change) => Self::ApprovalForAll {
                collection: change.collection.into(),
                owner: change.owner,
                operator: change.operator,
                approved: change.approved,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy::primitives::{Address, B256, Bytes, U256};
    use serde_json::json;

    use crate::interface as rpc;

    #[test]
    fn service_output_maps_into_rpc_response() {
        let output = simulation_service::SimulateEvmTransactionOutput {
            execution: simulation_service::SimulationExecution {
                chain_id: 1,
                block: simulation_service::SimulatedBlock {
                    number: 0x1234,
                    hash: B256::from_str(
                        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("hash"),
                },
                status: simulation_service::ExecutionStatus::Success,
                gas_used: 0x5208,
                gas_limit: 0x5300,
                output: Bytes::from_str("0x0102").expect("bytes"),
                failure: None,
            },
            changes: vec![simulation_service::Change::Transfer(
                simulation_service::TransferChange {
                    asset: simulation_service::Asset::Erc20 {
                        contract_address: Address::from_str(
                            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        )
                        .expect("token"),
                        display: Some(simulation_service::Erc20AssetDisplay {
                            name: Some("USD Coin".to_string()),
                            symbol: Some("USDC".to_string()),
                            decimals: Some(6),
                        }),
                    },
                    from: Address::from_str("0x1111111111111111111111111111111111111111")
                        .expect("from"),
                    to: Address::from_str("0x2222222222222222222222222222222222222222")
                        .expect("to"),
                    amount: Some(U256::from(0xde0b6b3a7640000_u64)),
                },
            )],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.execution.chain_id, 1);
        assert_eq!(response.execution.block.number, 0x1234);
        assert_eq!(response.execution.gas_used, 0x5208);
        assert_eq!(
            response.execution.output,
            Bytes::from_str("0x0102").unwrap()
        );
        assert_eq!(response.changes.len(), 1);

        let rpc::Change::Transfer { asset, amount, .. } = &response.changes[0] else {
            panic!("expected transfer change");
        };

        assert_eq!(*amount, Some(U256::from(0xde0b6b3a7640000_u64)));

        let rpc::Asset::Erc20 { display, .. } = asset else {
            panic!("expected erc20 asset");
        };

        let display = display.as_ref().expect("expected erc20 display");
        assert_eq!(display.name.as_deref(), Some("USD Coin"));
        assert_eq!(display.symbol.as_deref(), Some("USDC"));
        assert_eq!(display.decimals, Some(6));

        let serialized = serde_json::to_value(&response).expect("response should serialize");
        assert_eq!(serialized["execution"]["chainId"], json!("0x1"));
        assert_eq!(serialized["execution"]["block"]["number"], json!("0x1234"));
        assert_eq!(serialized["execution"]["gasUsed"], json!("0x5208"));
        assert_eq!(serialized["execution"]["output"], json!("0x0102"));
        assert_eq!(
            serialized["changes"][0]["amount"],
            json!("0xde0b6b3a7640000")
        );
    }

    #[test]
    fn erc721_transfer_asset_maps_collection_metadata_into_rpc_response() {
        let token =
            Address::from_str("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("erc721");
        let output = simulation_service::SimulateEvmTransactionOutput {
            execution: simulation_service::SimulationExecution {
                chain_id: 1,
                block: simulation_service::SimulatedBlock {
                    number: 1,
                    hash: B256::ZERO,
                },
                status: simulation_service::ExecutionStatus::Success,
                gas_used: 21_000,
                gas_limit: 50_000,
                output: Bytes::new(),
                failure: None,
            },
            changes: vec![simulation_service::Change::Transfer(
                simulation_service::TransferChange {
                    asset: simulation_service::Asset::Erc721 {
                        contract_address: token,
                        token_id: U256::from(42_u64),
                        collection: Some(simulation_service::Erc721CollectionDisplay {
                            name: Some("Mock NFT Collection".to_string()),
                            symbol: Some("MNFT".to_string()),
                        }),
                        token: None,
                    },
                    from: Address::from_str("0x1111111111111111111111111111111111111111")
                        .expect("from"),
                    to: Address::from_str("0x2222222222222222222222222222222222222222")
                        .expect("to"),
                    amount: None,
                },
            )],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        let rpc::Change::Transfer { asset, amount, .. } = &response.changes[0] else {
            panic!("expected transfer change");
        };

        assert!(amount.is_none());
        assert!(matches!(
            asset,
            rpc::Asset::Erc721 {
                contract_address,
                token_id,
                collection: Some(rpc::Erc721CollectionDisplay {
                    name: Some(name),
                    symbol: Some(symbol),
                }),
                token: None,
            } if contract_address == &token
                && token_id == &U256::from(42_u64)
                && name == "Mock NFT Collection"
                && symbol == "MNFT"
        ));

        let serialized = serde_json::to_value(&response).expect("response should serialize");
        assert_eq!(
            serialized["changes"][0]["asset"]["contractAddress"],
            json!("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(serialized["changes"][0]["asset"]["tokenId"], json!("0x2a"));
    }

    #[test]
    fn failed_service_output_maps_error_into_execution() {
        let output = simulation_service::SimulateEvmTransactionOutput {
            execution: simulation_service::SimulationExecution {
                chain_id: 1,
                block: simulation_service::SimulatedBlock {
                    number: 0x1234,
                    hash: B256::from_str(
                        "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    )
                    .expect("hash"),
                },
                status: simulation_service::ExecutionStatus::Failed,
                gas_used: 0x5208,
                gas_limit: 0x5300,
                output: Bytes::new(),
                failure: Some(simulation_service::ExecutionFailure {
                    code: "REVERT".to_string(),
                    message: "execution reverted".to_string(),
                    reason: Some("insufficient output".to_string()),
                }),
            },
            changes: Vec::new(),
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.execution.status, rpc::SimulationStatus::Failed);

        let error = response.execution.error.expect("expected execution error");
        assert_eq!(error.code, "REVERT");
        assert_eq!(error.message, "execution reverted");
        assert_eq!(error.reason.as_deref(), Some("insufficient output"));
    }

    #[test]
    fn approval_changes_map_into_rpc_response() {
        let owner = Address::from_str("0x1111111111111111111111111111111111111111").expect("owner");
        let spender =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("spender");
        let operator =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("operator");
        let erc20 = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("erc20");
        let erc1155 =
            Address::from_str("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").expect("erc1155");

        let output = simulation_service::SimulateEvmTransactionOutput {
            execution: simulation_service::SimulationExecution {
                chain_id: 1,
                block: simulation_service::SimulatedBlock {
                    number: 1,
                    hash: B256::ZERO,
                },
                status: simulation_service::ExecutionStatus::Success,
                gas_used: 21_000,
                gas_limit: 50_000,
                output: Bytes::new(),
                failure: None,
            },
            changes: vec![
                simulation_service::Change::Approval(simulation_service::ApprovalChange {
                    asset: simulation_service::Asset::Erc20 {
                        contract_address: erc20,
                        display: Some(simulation_service::Erc20AssetDisplay {
                            name: Some("USD Coin".to_string()),
                            symbol: Some("USDC".to_string()),
                            decimals: Some(6),
                        }),
                    },
                    owner,
                    spender,
                    amount: Some(U256::from(10_u64)),
                }),
                simulation_service::Change::ApprovalForAll(
                    simulation_service::ApprovalForAllChange {
                        collection: simulation_service::Collection::Erc1155 {
                            contract_address: erc1155,
                            collection: Some(simulation_service::Erc1155CollectionDisplay {
                                name: Some("Collection".to_string()),
                            }),
                        },
                        owner,
                        operator,
                        approved: true,
                    },
                ),
            ],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.changes.len(), 2);

        let rpc::Change::Approval {
            asset,
            owner: rpc_owner,
            spender: rpc_spender,
            amount,
        } = &response.changes[0]
        else {
            panic!("expected approval change");
        };
        assert_eq!(rpc_owner, &owner);
        assert_eq!(rpc_spender, &spender);
        assert_eq!(*amount, Some(U256::from(10_u64)));
        assert!(matches!(
            asset,
            rpc::Asset::Erc20 {
                contract_address,
                display: Some(rpc::Erc20AssetDisplay {
                    name: Some(name),
                    symbol: Some(symbol),
                    decimals: Some(6),
                }),
            } if contract_address == &erc20
                && name == "USD Coin"
                && symbol == "USDC"
        ));

        let rpc::Change::ApprovalForAll {
            collection,
            owner: rpc_owner,
            operator: rpc_operator,
            approved,
        } = &response.changes[1]
        else {
            panic!("expected approval for all change");
        };
        assert_eq!(rpc_owner, &owner);
        assert_eq!(rpc_operator, &operator);
        assert!(*approved);
        assert!(matches!(
            collection,
            rpc::Collection::Erc1155 {
                contract_address,
                collection: Some(rpc::Erc1155CollectionDisplay {
                    name: Some(name),
                }),
            } if contract_address == &erc1155 && name == "Collection"
        ));

        let serialized = serde_json::to_value(&response).expect("response should serialize");
        assert_eq!(
            serialized["changes"][0]["owner"],
            json!("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            serialized["changes"][0]["spender"],
            json!("0x2222222222222222222222222222222222222222")
        );
        assert_eq!(serialized["changes"][0]["amount"], json!("0xa"));
        assert_eq!(
            serialized["changes"][1]["operator"],
            json!("0x3333333333333333333333333333333333333333")
        );
    }
}

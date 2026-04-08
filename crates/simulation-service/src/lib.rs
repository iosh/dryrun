mod error;
mod types;

use std::sync::Arc;

use evm_engine::{EvmEngine, EvmExecutionInput, EvmSimulation};

pub use error::SimulationServiceError;
pub use types::{
    AccessListItem, ApprovalChange, ApprovalForAllChange, Asset, BlockRef, BurnChange, Change,
    Collection, Erc20AssetDisplay, Erc721CollectionDisplay, Erc1155CollectionDisplay,
    EvmTransaction, EvmTransactionType, ExecutionFailure, ExecutionStatus, MintChange,
    NativeAssetDisplay, NftTokenDisplay, SimulateEvmTransactionInput, SimulateEvmTransactionOutput,
    SimulatedBlock, SimulationExecution, TransferChange,
};

#[derive(Debug, Clone)]
pub struct SimulationService {
    evm_engine: Arc<EvmEngine>,
}

impl SimulationService {
    pub fn new(evm_engine: Arc<EvmEngine>) -> Self {
        Self { evm_engine }
    }

    pub async fn simulate_evm_transaction(
        &self,
        input: SimulateEvmTransactionInput,
    ) -> Result<SimulateEvmTransactionOutput, SimulationServiceError> {
        let simulation = self.evm_engine.simulate(input.into()).await?;
        Ok(simulation.into())
    }
}

impl From<SimulateEvmTransactionInput> for EvmExecutionInput {
    fn from(input: SimulateEvmTransactionInput) -> Self {
        Self {
            block: input.block.into(),
            transaction: input.transaction.into(),
        }
    }
}

impl From<BlockRef> for evm_engine::BlockRef {
    fn from(block: BlockRef) -> Self {
        match block {
            BlockRef::Latest => Self::Latest,
            BlockRef::Number(number) => Self::Number(number),
            BlockRef::Hash(hash) => Self::Hash(hash),
        }
    }
}

impl From<AccessListItem> for evm_engine::AccessListItem {
    fn from(item: AccessListItem) -> Self {
        Self {
            address: item.address,
            storage_keys: item.storage_keys,
        }
    }
}

impl From<EvmTransactionType> for evm_engine::EvmTransactionType {
    fn from(tx_type: EvmTransactionType) -> Self {
        match tx_type {
            EvmTransactionType::Legacy => Self::Legacy,
            EvmTransactionType::AccessList => Self::AccessList,
            EvmTransactionType::DynamicFee => Self::DynamicFee,
        }
    }
}

impl From<EvmTransaction> for evm_engine::EvmTransaction {
    fn from(transaction: EvmTransaction) -> Self {
        Self {
            tx_type: transaction.tx_type.into(),
            requested_chain_id: transaction.requested_chain_id,
            from: transaction.from,
            to: transaction.to,
            nonce: transaction.nonce,
            gas_limit: transaction.gas_limit,
            value: transaction.value,
            data: transaction.data,
            access_list: transaction
                .access_list
                .into_iter()
                .map(Into::into)
                .collect(),
            gas_price: transaction.gas_price,
            max_fee_per_gas: transaction.max_fee_per_gas,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
        }
    }
}

impl From<EvmSimulation> for SimulateEvmTransactionOutput {
    fn from(simulation: EvmSimulation) -> Self {
        let (execution, changes) = simulation.into_parts();

        Self {
            execution: execution.into(),
            changes: changes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<evm_engine::EvmExecutionStatus> for ExecutionStatus {
    fn from(status: evm_engine::EvmExecutionStatus) -> Self {
        match status {
            evm_engine::EvmExecutionStatus::Success => Self::Success,
            evm_engine::EvmExecutionStatus::Failed => Self::Failed,
        }
    }
}

impl From<evm_engine::SimulatedBlock> for SimulatedBlock {
    fn from(block: evm_engine::SimulatedBlock) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<evm_engine::EvmExecutionFailure> for ExecutionFailure {
    fn from(failure: evm_engine::EvmExecutionFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<evm_engine::EvmExecution> for SimulationExecution {
    fn from(execution: evm_engine::EvmExecution) -> Self {
        Self {
            chain_id: execution.chain_id,
            block: execution.block.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            output: execution.output,
            failure: execution.failure.map(Into::into),
        }
    }
}

impl From<evm_engine::NativeAssetDisplay> for NativeAssetDisplay {
    fn from(display: evm_engine::NativeAssetDisplay) -> Self {
        Self {
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<evm_engine::Erc20AssetDisplay> for Erc20AssetDisplay {
    fn from(display: evm_engine::Erc20AssetDisplay) -> Self {
        Self {
            name: display.name,
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<evm_engine::Erc721CollectionDisplay> for Erc721CollectionDisplay {
    fn from(collection: evm_engine::Erc721CollectionDisplay) -> Self {
        Self {
            name: collection.name,
            symbol: collection.symbol,
        }
    }
}

impl From<evm_engine::Erc1155CollectionDisplay> for Erc1155CollectionDisplay {
    fn from(collection: evm_engine::Erc1155CollectionDisplay) -> Self {
        Self {
            name: collection.name,
        }
    }
}

impl From<evm_engine::NftTokenDisplay> for NftTokenDisplay {
    fn from(token: evm_engine::NftTokenDisplay) -> Self {
        Self { name: token.name }
    }
}

impl From<evm_engine::Asset> for Asset {
    fn from(asset: evm_engine::Asset) -> Self {
        match asset {
            evm_engine::Asset::Native { display } => Self::Native {
                display: display.map(Into::into),
            },
            evm_engine::Asset::Erc20 {
                contract_address,
                display,
            } => Self::Erc20 {
                contract_address,
                display: display.map(Into::into),
            },
            evm_engine::Asset::Erc721 {
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
            evm_engine::Asset::Erc1155 {
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

impl From<evm_engine::Collection> for Collection {
    fn from(collection: evm_engine::Collection) -> Self {
        match collection {
            evm_engine::Collection::Erc721 {
                contract_address,
                collection,
            } => Self::Erc721 {
                contract_address,
                collection: collection.map(Into::into),
            },
            evm_engine::Collection::Erc1155 {
                contract_address,
                collection,
            } => Self::Erc1155 {
                contract_address,
                collection: collection.map(Into::into),
            },
        }
    }
}

impl From<evm_engine::TransferChange> for TransferChange {
    fn from(change: evm_engine::TransferChange) -> Self {
        Self {
            asset: change.asset.into(),
            from: change.from,
            to: change.to,
            amount: change.amount,
        }
    }
}

impl From<evm_engine::MintChange> for MintChange {
    fn from(change: evm_engine::MintChange) -> Self {
        Self {
            asset: change.asset.into(),
            to: change.to,
            amount: change.amount,
        }
    }
}

impl From<evm_engine::BurnChange> for BurnChange {
    fn from(change: evm_engine::BurnChange) -> Self {
        Self {
            asset: change.asset.into(),
            from: change.from,
            amount: change.amount,
        }
    }
}

impl From<evm_engine::ApprovalChange> for ApprovalChange {
    fn from(change: evm_engine::ApprovalChange) -> Self {
        Self {
            asset: change.asset.into(),
            owner: change.owner,
            spender: change.spender,
            amount: change.amount,
        }
    }
}

impl From<evm_engine::ApprovalForAllChange> for ApprovalForAllChange {
    fn from(change: evm_engine::ApprovalForAllChange) -> Self {
        Self {
            collection: change.collection.into(),
            owner: change.owner,
            operator: change.operator,
            approved: change.approved,
        }
    }
}

impl From<evm_engine::Change> for Change {
    fn from(change: evm_engine::Change) -> Self {
        match change {
            evm_engine::Change::Transfer(change) => Self::Transfer(change.into()),
            evm_engine::Change::Mint(change) => Self::Mint(change.into()),
            evm_engine::Change::Burn(change) => Self::Burn(change.into()),
            evm_engine::Change::Approval(change) => Self::Approval(change.into()),
            evm_engine::Change::ApprovalForAll(change) => Self::ApprovalForAll(change.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};
    use evm_engine::{EvmExecution, EvmSimulation};

    use super::*;

    #[test]
    fn engine_simulation_maps_into_service_output() {
        let simulation = EvmSimulation::new(
            EvmExecution {
                chain_id: 1,
                block: evm_engine::SimulatedBlock {
                    number: 0x1234,
                    hash: B256::from_str(
                        "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    )
                    .expect("hash"),
                },
                status: evm_engine::EvmExecutionStatus::Success,
                gas_used: 0x5208,
                gas_limit: 0x5300,
                output: Bytes::from_str("0x0102").expect("bytes"),
                failure: None,
            },
            vec![evm_engine::Change::Transfer(evm_engine::TransferChange {
                asset: evm_engine::Asset::Erc20 {
                    contract_address: Address::from_str(
                        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                    )
                    .expect("token"),
                    display: Some(evm_engine::Erc20AssetDisplay {
                        name: Some("USD Coin".to_string()),
                        symbol: Some("USDC".to_string()),
                        decimals: Some(6),
                    }),
                },
                from: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("from"),
                to: Address::from_str("0x2222222222222222222222222222222222222222").expect("to"),
                amount: Some(U256::from(0x1234_u64)),
            })],
        );

        let mapped: SimulateEvmTransactionOutput = simulation.into();
        assert_eq!(mapped.execution.chain_id, 1);
        assert_eq!(mapped.execution.gas_used, 0x5208);
        assert_eq!(mapped.changes.len(), 1);

        let Change::Transfer(change) = &mapped.changes[0] else {
            panic!("expected transfer change");
        };

        assert_eq!(change.amount, Some(U256::from(0x1234_u64)));

        let Asset::Erc20 { display, .. } = &change.asset else {
            panic!("expected erc20 asset");
        };

        let display = display.as_ref().expect("expected erc20 display");
        assert_eq!(display.name.as_deref(), Some("USD Coin"));
        assert_eq!(display.symbol.as_deref(), Some("USDC"));
        assert_eq!(display.decimals, Some(6));
    }

    #[test]
    fn engine_erc721_transfer_maps_collection_metadata_into_service_output() {
        let token =
            Address::from_str("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("erc721");
        let from = Address::from_str("0x1111111111111111111111111111111111111111").expect("from");
        let to = Address::from_str("0x2222222222222222222222222222222222222222").expect("to");

        let simulation = EvmSimulation::new(
            EvmExecution {
                chain_id: 1,
                block: evm_engine::SimulatedBlock {
                    number: 1,
                    hash: B256::ZERO,
                },
                status: evm_engine::EvmExecutionStatus::Success,
                gas_used: 21_000,
                gas_limit: 50_000,
                output: Bytes::new(),
                failure: None,
            },
            vec![evm_engine::Change::Transfer(evm_engine::TransferChange {
                asset: evm_engine::Asset::Erc721 {
                    contract_address: token,
                    token_id: U256::from(42_u64),
                    collection: Some(evm_engine::Erc721CollectionDisplay {
                        name: Some("Mock NFT Collection".to_string()),
                        symbol: Some("MNFT".to_string()),
                    }),
                    token: None,
                },
                from,
                to,
                amount: None,
            })],
        );

        let mapped: SimulateEvmTransactionOutput = simulation.into();
        let Change::Transfer(change) = &mapped.changes[0] else {
            panic!("expected transfer change");
        };

        assert!(matches!(
            &change.asset,
            Asset::Erc721 {
                contract_address,
                token_id,
                collection: Some(Erc721CollectionDisplay {
                    name: Some(name),
                    symbol: Some(symbol),
                }),
                token: None,
            } if *contract_address == token
                && *token_id == U256::from(42_u64)
                && name == "Mock NFT Collection"
                && symbol == "MNFT"
        ));
    }

    #[test]
    fn engine_approval_changes_map_into_service_output() {
        let owner = Address::from_str("0x1111111111111111111111111111111111111111").expect("owner");
        let spender =
            Address::from_str("0x2222222222222222222222222222222222222222").expect("spender");
        let operator =
            Address::from_str("0x3333333333333333333333333333333333333333").expect("operator");
        let erc20 = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").expect("erc20");
        let erc721 =
            Address::from_str("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").expect("erc721");

        let simulation = EvmSimulation::new(
            EvmExecution {
                chain_id: 1,
                block: evm_engine::SimulatedBlock {
                    number: 1,
                    hash: B256::ZERO,
                },
                status: evm_engine::EvmExecutionStatus::Success,
                gas_used: 21_000,
                gas_limit: 50_000,
                output: Bytes::new(),
                failure: None,
            },
            vec![
                evm_engine::Change::Approval(evm_engine::ApprovalChange {
                    asset: evm_engine::Asset::Erc20 {
                        contract_address: erc20,
                        display: Some(evm_engine::Erc20AssetDisplay {
                            name: Some("USD Coin".to_string()),
                            symbol: Some("USDC".to_string()),
                            decimals: Some(6),
                        }),
                    },
                    owner,
                    spender,
                    amount: Some(U256::from(9_u64)),
                }),
                evm_engine::Change::ApprovalForAll(evm_engine::ApprovalForAllChange {
                    collection: evm_engine::Collection::Erc721 {
                        contract_address: erc721,
                        collection: Some(evm_engine::Erc721CollectionDisplay {
                            name: None,
                            symbol: Some("NFT".to_string()),
                        }),
                    },
                    owner,
                    operator,
                    approved: false,
                }),
            ],
        );

        let mapped: SimulateEvmTransactionOutput = simulation.into();
        assert_eq!(mapped.changes.len(), 2);

        let Change::Approval(approval) = &mapped.changes[0] else {
            panic!("expected approval change");
        };
        assert_eq!(approval.owner, owner);
        assert_eq!(approval.spender, spender);
        assert_eq!(approval.amount, Some(U256::from(9_u64)));
        assert!(matches!(
            &approval.asset,
            Asset::Erc20 {
                contract_address,
                display: Some(Erc20AssetDisplay {
                    name: Some(name),
                    symbol: Some(symbol),
                    decimals: Some(6),
                }),
            } if *contract_address == erc20 && name == "USD Coin" && symbol == "USDC"
        ));

        let Change::ApprovalForAll(approval_for_all) = &mapped.changes[1] else {
            panic!("expected approval for all change");
        };
        assert_eq!(approval_for_all.owner, owner);
        assert_eq!(approval_for_all.operator, operator);
        assert!(!approval_for_all.approved);
        assert!(matches!(
            &approval_for_all.collection,
            Collection::Erc721 {
                contract_address,
                collection: Some(Erc721CollectionDisplay {
                    name: None,
                    symbol: Some(symbol),
                }),
            } if *contract_address == erc721 && symbol == "NFT"
        ));
    }
}

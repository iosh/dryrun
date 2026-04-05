use crate::interface as rpc;

use super::primitives::{format_u64_quantity, format_u256_quantity};

impl From<simulation_service::SimulateEvmTransactionOutput>
    for rpc::EvmSimulateTransactionResponse
{
    fn from(output: simulation_service::SimulateEvmTransactionOutput) -> Self {
        Self {
            execution: rpc::Execution {
                chain_id: format_u64_quantity(output.chain_id),
                block: output.block.into(),
                status: output.status.into(),
                gas_used: format_u64_quantity(output.gas_used),
                gas_limit: format_u64_quantity(output.gas_limit),
                output: format!("{:#x}", output.output),
                error: output.error.map(Into::into),
            },
            changes: output.changes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<simulation_service::SimulatedBlock> for rpc::SimulatedBlock {
    fn from(block: simulation_service::SimulatedBlock) -> Self {
        Self {
            number: format_u64_quantity(block.number),
            hash: format!("{:#x}", block.hash),
        }
    }
}

impl From<simulation_service::SimulationStatus> for rpc::SimulationStatus {
    fn from(status: simulation_service::SimulationStatus) -> Self {
        match status {
            simulation_service::SimulationStatus::Success => Self::Success,
            simulation_service::SimulationStatus::Failed => Self::Failed,
        }
    }
}

impl From<simulation_service::SimulationError> for rpc::ExecutionError {
    fn from(error: simulation_service::SimulationError) -> Self {
        Self {
            code: error.code,
            message: error.message,
            reason: error.reason,
        }
    }
}

impl From<simulation_service::Collection> for rpc::Collection {
    fn from(collection: simulation_service::Collection) -> Self {
        match collection {
            simulation_service::Collection::Erc721 {
                contract_address,
                collection_name,
                name,
                symbol,
            } => Self::Erc721 {
                contract_address: format!("{:#x}", contract_address),
                collection_name,
                name,
                symbol,
            },
            simulation_service::Collection::Erc1155 {
                contract_address,
                collection_name,
                name,
                symbol,
            } => Self::Erc1155 {
                contract_address: format!("{:#x}", contract_address),
                collection_name,
                name,
                symbol,
            },
        }
    }
}

impl From<simulation_service::Asset> for rpc::Asset {
    fn from(asset: simulation_service::Asset) -> Self {
        match asset {
            simulation_service::Asset::Native { symbol, decimals } => {
                Self::Native { symbol, decimals }
            }
            simulation_service::Asset::Erc20 {
                contract_address,
                symbol,
                decimals,
                name,
            } => Self::Erc20 {
                contract_address: format!("{:#x}", contract_address),
                symbol,
                decimals,
                name,
            },
            simulation_service::Asset::Erc721 {
                contract_address,
                token_id,
                collection_name,
                name,
                symbol,
            } => Self::Erc721 {
                contract_address: format!("{:#x}", contract_address),
                token_id: format_u256_quantity(token_id),
                collection_name,
                name,
                symbol,
            },
            simulation_service::Asset::Erc1155 {
                contract_address,
                token_id,
                collection_name,
                name,
                symbol,
            } => Self::Erc1155 {
                contract_address: format!("{:#x}", contract_address),
                token_id: format_u256_quantity(token_id),
                collection_name,
                name,
                symbol,
            },
        }
    }
}

impl From<simulation_service::Change> for rpc::Change {
    fn from(change: simulation_service::Change) -> Self {
        match change {
            simulation_service::Change::Transfer(change) => Self::Transfer {
                asset: change.asset.into(),
                from: format!("{:#x}", change.from),
                to: format!("{:#x}", change.to),
                amount: change.amount.map(format_u256_quantity),
            },
            simulation_service::Change::Mint(change) => Self::Mint {
                asset: change.asset.into(),
                to: format!("{:#x}", change.to),
                amount: change.amount.map(format_u256_quantity),
            },
            simulation_service::Change::Burn(change) => Self::Burn {
                asset: change.asset.into(),
                from: format!("{:#x}", change.from),
                amount: change.amount.map(format_u256_quantity),
            },
            simulation_service::Change::Approval(change) => Self::Approval {
                asset: change.asset.into(),
                owner: format!("{:#x}", change.owner),
                spender: format!("{:#x}", change.spender),
                amount: change.amount.map(format_u256_quantity),
            },
            simulation_service::Change::ApprovalForAll(change) => Self::ApprovalForAll {
                collection: change.collection.into(),
                owner: format!("{:#x}", change.owner),
                operator: format!("{:#x}", change.operator),
                approved: change.approved,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy::primitives::{Address, B256, Bytes, U256};

    use crate::interface as rpc;

    #[test]
    fn service_output_maps_into_rpc_response() {
        let output = simulation_service::SimulateEvmTransactionOutput {
            chain_id: 1,
            block: simulation_service::SimulatedBlock {
                number: 0x1234,
                hash: B256::from_str(
                    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                )
                .expect("hash"),
            },
            status: simulation_service::SimulationStatus::Success,
            gas_used: 0x5208,
            gas_limit: 0x5300,
            output: Bytes::from_str("0x0102").expect("bytes"),
            error: None,
            changes: vec![simulation_service::Change::Transfer(
                simulation_service::TransferChange {
                    asset: simulation_service::Asset::Erc20 {
                        contract_address: Address::from_str(
                            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        )
                        .expect("token"),
                        symbol: Some("USDC".to_string()),
                        decimals: Some(6),
                        name: None,
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
        assert_eq!(response.execution.chain_id, "0x1");
        assert_eq!(response.execution.block.number, "0x1234");
        assert_eq!(response.execution.gas_used, "0x5208");
        assert_eq!(response.execution.output, "0x0102");
        assert_eq!(response.changes.len(), 1);

        let rpc::Change::Transfer { asset, amount, .. } = &response.changes[0] else {
            panic!("expected transfer change");
        };

        assert_eq!(amount.as_deref(), Some("0xde0b6b3a7640000"));

        let rpc::Asset::Erc20 {
            symbol, decimals, ..
        } = asset
        else {
            panic!("expected erc20 asset");
        };

        assert_eq!(symbol.as_deref(), Some("USDC"));
        assert_eq!(*decimals, Some(6));
    }

    #[test]
    fn failed_service_output_maps_error_into_execution() {
        let output = simulation_service::SimulateEvmTransactionOutput {
            chain_id: 1,
            block: simulation_service::SimulatedBlock {
                number: 0x1234,
                hash: B256::from_str(
                    "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                )
                .expect("hash"),
            },
            status: simulation_service::SimulationStatus::Failed,
            gas_used: 0x5208,
            gas_limit: 0x5300,
            output: Bytes::new(),
            error: Some(simulation_service::SimulationError {
                code: "REVERT".to_string(),
                message: "execution reverted".to_string(),
                reason: Some("insufficient output".to_string()),
            }),
            changes: Vec::new(),
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.execution.status, rpc::SimulationStatus::Failed);

        let error = response.execution.error.expect("expected execution error");
        assert_eq!(error.code, "REVERT");
        assert_eq!(error.message, "execution reverted");
        assert_eq!(error.reason.as_deref(), Some("insufficient output"));
    }
}

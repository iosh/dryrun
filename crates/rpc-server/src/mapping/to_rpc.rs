use crate::interface as rpc;

use super::primitives::{format_u64_quantity, format_u256_quantity};

impl From<simulation_service::SimulateEvmTransactionOutput>
    for rpc::EvmSimulateTransactionResponse
{
    fn from(output: simulation_service::SimulateEvmTransactionOutput) -> Self {
        Self {
            chain_id: format_u64_quantity(output.chain_id),
            block: output.block.into(),
            status: output.status.into(),
            gas_used: format_u64_quantity(output.gas_used),
            gas_limit: format_u64_quantity(output.gas_limit),
            output: format!("{:#x}", output.output),
            failure: output.failure.map(Into::into),
            logs: output.logs.into_iter().map(Into::into).collect(),
            asset_changes: output.asset_changes.into_iter().map(Into::into).collect(),
            trace: output.trace.into_iter().map(Into::into).collect(),
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

impl From<simulation_service::SimulationFailure> for rpc::SimulationFailure {
    fn from(failure: simulation_service::SimulationFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<simulation_service::RawLog> for rpc::RawLog {
    fn from(log: simulation_service::RawLog) -> Self {
        Self {
            log_index: format_u64_quantity(log.log_index),
            address: format!("{:#x}", log.address),
            topics: log
                .topics
                .into_iter()
                .map(|topic| format!("{:#x}", topic))
                .collect(),
            data: format!("{:#x}", log.data),
        }
    }
}

impl From<simulation_service::AssetType> for rpc::AssetType {
    fn from(asset_type: simulation_service::AssetType) -> Self {
        match asset_type {
            simulation_service::AssetType::Native => Self::Native,
            simulation_service::AssetType::Erc20 => Self::Erc20,
        }
    }
}

impl From<simulation_service::AssetChangeType> for rpc::AssetChangeType {
    fn from(change_type: simulation_service::AssetChangeType) -> Self {
        match change_type {
            simulation_service::AssetChangeType::Transfer => Self::Transfer,
        }
    }
}

impl From<simulation_service::AssetChangeAsset> for rpc::AssetChangeAsset {
    fn from(asset: simulation_service::AssetChangeAsset) -> Self {
        Self {
            token_address: format!("{:#x}", asset.token_address),
            symbol: asset.symbol,
            decimals: asset.decimals,
        }
    }
}

impl From<simulation_service::AssetChange> for rpc::AssetChange {
    fn from(asset_change: simulation_service::AssetChange) -> Self {
        Self {
            asset_type: asset_change.asset_type.into(),
            change_type: asset_change.change_type.into(),
            from: format!("{:#x}", asset_change.from),
            to: format!("{:#x}", asset_change.to),
            amount: format_u256_quantity(asset_change.amount),
            asset: asset_change.asset.map(Into::into),
        }
    }
}

impl From<simulation_service::TraceType> for rpc::TraceType {
    fn from(trace_type: simulation_service::TraceType) -> Self {
        match trace_type {
            simulation_service::TraceType::Call => Self::Call,
            simulation_service::TraceType::CallCode => Self::CallCode,
            simulation_service::TraceType::DelegateCall => Self::DelegateCall,
            simulation_service::TraceType::StaticCall => Self::StaticCall,
            simulation_service::TraceType::Create => Self::Create,
            simulation_service::TraceType::Create2 => Self::Create2,
        }
    }
}

impl From<simulation_service::TraceStatus> for rpc::TraceStatus {
    fn from(status: simulation_service::TraceStatus) -> Self {
        match status {
            simulation_service::TraceStatus::Success => Self::Success,
            simulation_service::TraceStatus::Revert => Self::Revert,
            simulation_service::TraceStatus::Halt => Self::Halt,
        }
    }
}

impl From<simulation_service::TraceItem> for rpc::TraceItem {
    fn from(trace: simulation_service::TraceItem) -> Self {
        Self {
            trace_type: trace.trace_type.into(),
            status: trace.status.into(),
            from: format!("{:#x}", trace.from),
            to: trace.to.map(|address| format!("{:#x}", address)),
            code_address: trace.code_address.map(|address| format!("{:#x}", address)),
            value: format_u256_quantity(trace.value),
            input: format!("{:#x}", trace.input),
            output: format!("{:#x}", trace.output),
            gas: format_u64_quantity(trace.gas),
            gas_used: format_u64_quantity(trace.gas_used),
            trace_address: trace
                .trace_address
                .into_iter()
                .map(format_u64_quantity)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};

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
            failure: None,
            logs: vec![simulation_service::RawLog {
                log_index: 0,
                address: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("address"),
                topics: vec![
                    B256::from_str(
                        "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    )
                    .expect("topic"),
                ],
                data: Bytes::from_str("0xdeadbeef").expect("bytes"),
            }],
            asset_changes: vec![simulation_service::AssetChange {
                asset_type: simulation_service::AssetType::Erc20,
                change_type: simulation_service::AssetChangeType::Transfer,
                from: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("from"),
                to: Address::from_str("0x2222222222222222222222222222222222222222").expect("to"),
                amount: U256::from(0xde0b6b3a7640000_u64),
                asset: Some(simulation_service::AssetChangeAsset {
                    token_address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                        .expect("token"),
                    symbol: Some("USDC".to_string()),
                    decimals: Some(6),
                }),
            }],
            trace: vec![simulation_service::TraceItem {
                trace_type: simulation_service::TraceType::Call,
                status: simulation_service::TraceStatus::Success,
                from: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("from"),
                to: Some(
                    Address::from_str("0x2222222222222222222222222222222222222222").expect("to"),
                ),
                code_address: Some(
                    Address::from_str("0x3333333333333333333333333333333333333333")
                        .expect("code address"),
                ),
                value: U256::from(0x1234_u64),
                input: Bytes::from_str("0x1234").expect("bytes"),
                output: Bytes::from_str("0xabcd").expect("bytes"),
                gas: 50_000,
                gas_used: 21_000,
                trace_address: vec![0],
            }],
        };

        let response: rpc::EvmSimulateTransactionResponse = output.into();
        assert_eq!(response.chain_id, "0x1");
        assert_eq!(response.block.number, "0x1234");
        assert_eq!(response.gas_used, "0x5208");
        assert_eq!(response.logs.len(), 1);
        assert_eq!(response.logs[0].log_index, "0x0");
        assert_eq!(response.asset_changes.len(), 1);
        assert_eq!(response.asset_changes[0].amount, "0xde0b6b3a7640000");
        assert_eq!(response.output, "0x0102");
        let asset = response.asset_changes[0]
            .asset
            .as_ref()
            .expect("erc20 asset");
        assert_eq!(
            asset.token_address,
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        );
        assert_eq!(asset.symbol.as_deref(), Some("USDC"));
        assert_eq!(asset.decimals, Some(6));
        assert_eq!(response.trace.len(), 1);
        assert_eq!(
            response.trace[0].code_address.as_deref(),
            Some("0x3333333333333333333333333333333333333333")
        );
        assert!(matches!(
            response.trace[0].status,
            rpc::TraceStatus::Success
        ));
        assert_eq!(response.trace[0].gas, "0xc350");
        assert_eq!(response.trace[0].gas_used, "0x5208");
        assert_eq!(response.trace[0].trace_address, vec!["0x0"]);
    }
}

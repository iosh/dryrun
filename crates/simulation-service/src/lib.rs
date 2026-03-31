mod error;
mod types;

use std::sync::Arc;

use evm_engine::{EvmEngine, EvmExecutionInput, EvmExecutionOutput};

pub use error::SimulationServiceError;
pub use types::{
    AccessListItem, AssetChange, AssetChangeAsset, AssetChangeType, AssetType, BlockRef,
    EvmTransaction, EvmTransactionType, RawLog, SimulateEvmTransactionInput,
    SimulateEvmTransactionOutput, SimulatedBlock, SimulationFailure, SimulationStatus, TraceItem,
    TraceStatus, TraceType,
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
        let SimulateEvmTransactionInput { block, transaction } = input;
        let output = self
            .evm_engine
            .simulate(EvmExecutionInput {
                block: block.into(),
                transaction: transaction.into(),
            })
            .await?;

        Ok(SimulateEvmTransactionOutput::from_engine_output(output))
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

impl From<EvmTransactionType> for evm_engine::EvmTransactionType {
    fn from(tx_type: EvmTransactionType) -> Self {
        match tx_type {
            EvmTransactionType::Legacy => Self::Legacy,
            EvmTransactionType::AccessList => Self::AccessList,
            EvmTransactionType::DynamicFee => Self::DynamicFee,
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

impl From<evm_engine::EvmExecutionStatus> for SimulationStatus {
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

impl From<evm_engine::EvmExecutionFailure> for SimulationFailure {
    fn from(failure: evm_engine::EvmExecutionFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<evm_engine::EvmExecutionLog> for RawLog {
    fn from(log: evm_engine::EvmExecutionLog) -> Self {
        Self {
            log_index: log.log_index,
            address: log.address,
            topics: log.topics,
            data: log.data,
        }
    }
}

impl From<evm_engine::AssetType> for AssetType {
    fn from(asset_type: evm_engine::AssetType) -> Self {
        match asset_type {
            evm_engine::AssetType::Native => Self::Native,
            evm_engine::AssetType::Erc20 => Self::Erc20,
        }
    }
}

impl From<evm_engine::AssetChangeType> for AssetChangeType {
    fn from(change_type: evm_engine::AssetChangeType) -> Self {
        match change_type {
            evm_engine::AssetChangeType::Transfer => Self::Transfer,
        }
    }
}

impl From<evm_engine::AssetChangeAsset> for AssetChangeAsset {
    fn from(asset: evm_engine::AssetChangeAsset) -> Self {
        Self {
            token_address: asset.token_address,
            symbol: asset.symbol,
            decimals: asset.decimals,
        }
    }
}

impl From<evm_engine::AssetChange> for AssetChange {
    fn from(asset_change: evm_engine::AssetChange) -> Self {
        Self {
            asset_type: asset_change.asset_type.into(),
            change_type: asset_change.change_type.into(),
            from: asset_change.from,
            to: asset_change.to,
            amount: asset_change.amount,
            asset: asset_change.asset.map(Into::into),
        }
    }
}

impl From<evm_engine::TraceType> for TraceType {
    fn from(trace_type: evm_engine::TraceType) -> Self {
        match trace_type {
            evm_engine::TraceType::Call => Self::Call,
            evm_engine::TraceType::CallCode => Self::CallCode,
            evm_engine::TraceType::DelegateCall => Self::DelegateCall,
            evm_engine::TraceType::StaticCall => Self::StaticCall,
            evm_engine::TraceType::Create => Self::Create,
            evm_engine::TraceType::Create2 => Self::Create2,
        }
    }
}

impl From<evm_engine::TraceStatus> for TraceStatus {
    fn from(status: evm_engine::TraceStatus) -> Self {
        match status {
            evm_engine::TraceStatus::Success => Self::Success,
            evm_engine::TraceStatus::Revert => Self::Revert,
            evm_engine::TraceStatus::Halt => Self::Halt,
        }
    }
}

impl From<evm_engine::TraceItem> for TraceItem {
    fn from(trace: evm_engine::TraceItem) -> Self {
        Self {
            trace_type: trace.trace_type.into(),
            status: trace.status.into(),
            from: trace.from,
            to: trace.to,
            code_address: trace.code_address,
            value: trace.value,
            input: trace.input,
            output: trace.output,
            gas: trace.gas,
            gas_used: trace.gas_used,
            trace_address: trace.trace_address,
        }
    }
}

impl SimulateEvmTransactionOutput {
    fn from_engine_output(output: EvmExecutionOutput) -> Self {
        let EvmExecutionOutput {
            chain_id,
            block,
            status,
            gas_used,
            gas_limit,
            output,
            failure,
            logs,
            asset_changes,
            trace,
        } = output;

        Self {
            chain_id,
            block: block.into(),
            status: status.into(),
            gas_used,
            gas_limit,
            output,
            failure: failure.map(Into::into),
            logs: logs.into_iter().map(Into::into).collect(),
            asset_changes: asset_changes.into_iter().map(Into::into).collect(),
            trace: trace.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};

    use super::*;

    #[test]
    fn engine_output_maps_into_service_output() {
        let output = EvmExecutionOutput {
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
            logs: vec![evm_engine::EvmExecutionLog {
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
            asset_changes: vec![evm_engine::AssetChange {
                asset_type: evm_engine::AssetType::Erc20,
                change_type: evm_engine::AssetChangeType::Transfer,
                from: Address::from_str("0x1111111111111111111111111111111111111111")
                    .expect("from"),
                to: Address::from_str("0x2222222222222222222222222222222222222222").expect("to"),
                amount: U256::from(0x1234_u64),
                asset: Some(evm_engine::AssetChangeAsset {
                    token_address: Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
                        .expect("token"),
                    symbol: Some("USDC".to_string()),
                    decimals: Some(6),
                }),
            }],
            trace: vec![evm_engine::TraceItem {
                trace_type: evm_engine::TraceType::Call,
                status: evm_engine::TraceStatus::Success,
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

        let mapped = SimulateEvmTransactionOutput::from_engine_output(output);
        assert_eq!(mapped.logs.len(), 1);
        assert_eq!(mapped.asset_changes.len(), 1);
        assert_eq!(mapped.trace.len(), 1);
        assert_eq!(mapped.asset_changes[0].amount, U256::from(0x1234_u64));
        let asset = mapped.asset_changes[0].asset.as_ref().expect("erc20 asset");
        assert_eq!(asset.symbol.as_deref(), Some("USDC"));
        assert_eq!(asset.decimals, Some(6));
        assert_eq!(
            mapped.trace[0].code_address,
            Some(
                Address::from_str("0x3333333333333333333333333333333333333333")
                    .expect("code address"),
            )
        );
        assert_eq!(mapped.trace[0].status, TraceStatus::Success);
        assert_eq!(mapped.trace[0].trace_address, vec![0]);
    }
}

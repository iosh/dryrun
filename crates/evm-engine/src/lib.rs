mod asset_changes;
mod error;
mod execution;
mod frames;
mod trace;
mod types;

pub use error::EvmEngineError;
use execution::simulate_execution;
pub use types::{
    AccessListItem, AssetChange, AssetChangeAsset, AssetChangeType, AssetType, BlockRef,
    EvmExecutionFailure, EvmExecutionInput, EvmExecutionLog, EvmExecutionOutput,
    EvmExecutionStatus, EvmTransaction, EvmTransactionType, SimulatedBlock, TraceItem, TraceStatus,
    TraceType,
};

#[derive(Debug, Clone)]
pub struct EvmEngine {
    rpc_url: String,
}

impl EvmEngine {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub async fn simulate(
        &self,
        input: EvmExecutionInput,
    ) -> Result<EvmExecutionOutput, EvmEngineError> {
        ensure_supported_block_ref(&input.block)?;
        ensure_supported_transaction_type(input.transaction.tx_type)?;

        simulate_execution(&self.rpc_url, input).await
    }
}

fn ensure_supported_block_ref(block: &BlockRef) -> Result<(), EvmEngineError> {
    match block {
        BlockRef::Latest | BlockRef::Number(_) => Ok(()),
        BlockRef::Hash(_) => Err(EvmEngineError::not_supported(
            "block.hash is not supported yet",
        )),
    }
}

fn ensure_supported_transaction_type(tx_type: EvmTransactionType) -> Result<(), EvmEngineError> {
    match tx_type {
        EvmTransactionType::Legacy
        | EvmTransactionType::AccessList
        | EvmTransactionType::DynamicFee => Ok(()),
    }
}

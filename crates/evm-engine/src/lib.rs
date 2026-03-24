mod error;
mod execution;
mod types;

pub use error::EvmEngineError;
pub use types::{
    AccessListItem, BlockRef, EvmExecutionFailure, EvmExecutionInput, EvmExecutionLog,
    EvmExecutionOutput, EvmExecutionStatus, EvmTransaction, EvmTransactionType, SimulatedBlock,
};

use execution::simulate_latest_dynamic_fee;

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

        simulate_latest_dynamic_fee(&self.rpc_url, input).await
    }
}

fn ensure_supported_block_ref(block: &BlockRef) -> Result<(), EvmEngineError> {
    match block {
        BlockRef::Latest => Ok(()),
        BlockRef::Number(_) => Err(EvmEngineError::not_ready("block.number is not implemented")),
        BlockRef::Hash(_) => Err(EvmEngineError::not_ready("block.hash is not implemented")),
    }
}

fn ensure_supported_transaction_type(tx_type: EvmTransactionType) -> Result<(), EvmEngineError> {
    match tx_type {
        EvmTransactionType::DynamicFee => Ok(()),
        EvmTransactionType::Legacy => Err(EvmEngineError::not_ready(
            "transaction.type=0x0 is not implemented",
        )),
        EvmTransactionType::AccessList => Err(EvmEngineError::not_ready(
            "transaction.type=0x1 is not implemented",
        )),
    }
}

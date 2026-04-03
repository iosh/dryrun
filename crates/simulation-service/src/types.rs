use alloy_primitives::Bytes;

pub use evm_engine::{
    AccessListItem, AssetChange, AssetChangeAsset, AssetChangeType, AssetType, BlockRef,
    EvmExecutionFailure as SimulationFailure, EvmExecutionStatus as SimulationStatus,
    EvmTransaction, EvmTransactionType, SimulatedBlock,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionInput {
    pub block: BlockRef,
    pub transaction: EvmTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionOutput {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: SimulationStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Bytes,
    pub failure: Option<SimulationFailure>,
    pub asset_changes: Vec<AssetChange>,
}

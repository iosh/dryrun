use alloy_primitives::Bytes;

pub use evm_engine::{
    AccessListItem, ApprovalChange, ApprovalForAllChange, Asset, BlockRef, BurnChange, Change,
    Collection, EvmExecutionFailure as SimulationError, EvmExecutionStatus as SimulationStatus,
    EvmTransaction, EvmTransactionType, MintChange, SimulatedBlock, TransferChange,
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
    pub error: Option<SimulationError>,
    pub changes: Vec<Change>,
}

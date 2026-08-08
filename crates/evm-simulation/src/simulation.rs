use alloy_primitives::B256;

use crate::{Change, CompleteTransaction, EvmExecutionOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmBlockContext {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulation {
    pub context: EvmBlockContext,
    pub transaction: CompleteTransaction,
    pub execution: EvmExecutionOutcome,
    pub changes: Vec<Change>,
}

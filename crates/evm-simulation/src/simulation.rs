use alloy_primitives::B256;

use crate::{CompleteTransaction, EvmChanges, EvmExecutionOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmBlockContext {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug)]
pub struct EvmSimulation {
    pub context: EvmBlockContext,
    pub transaction: CompleteTransaction,
    pub execution: EvmExecutionOutcome,
    pub changes: EvmChanges,
}

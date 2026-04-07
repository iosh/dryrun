use alloy_primitives::{B256, Bytes};

use crate::Change;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: EvmExecutionStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Bytes,
    pub failure: Option<EvmExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulation {
    execution: EvmExecution,
    changes: Vec<Change>,
}

impl EvmSimulation {
    pub fn new(execution: EvmExecution, changes: Vec<Change>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &EvmExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn into_parts(self) -> (EvmExecution, Vec<Change>) {
        (self.execution, self.changes)
    }
}

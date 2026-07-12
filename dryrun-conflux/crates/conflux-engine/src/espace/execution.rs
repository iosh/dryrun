use cfx_bytes::Bytes;
use cfx_types::{H256, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: EspaceExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
    pub failure: Option<EspaceExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulation {
    execution: EspaceExecution,
}

impl EspaceSimulation {
    pub fn new(execution: EspaceExecution) -> Self {
        Self { execution }
    }

    pub fn execution(&self) -> &EspaceExecution {
        &self.execution
    }

    pub fn into_execution(self) -> EspaceExecution {
        self.execution
    }
}

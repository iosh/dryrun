use alloy_primitives::{Address, B256, Bytes};

use crate::{
    EvmExecutionFailure, EvmExecutionStatus, SimulatedBlock,
    change_observer::ObservedChange,
    frames::ExecutionFrame,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawExecutionLog {
    pub(crate) log_index: u64,
    pub(crate) address: Address,
    pub(crate) topics: Vec<B256>,
    pub(crate) data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionArtifacts {
    pub(crate) chain_id: u64,
    pub(crate) block: SimulatedBlock,
    pub(crate) status: EvmExecutionStatus,
    pub(crate) gas_used: u64,
    pub(crate) gas_limit: u64,
    pub(crate) output: Bytes,
    pub(crate) failure: Option<EvmExecutionFailure>,
    pub(crate) observed_changes: Vec<ObservedChange>,
    pub(crate) logs: Vec<RawExecutionLog>,
    pub(crate) frames: Vec<ExecutionFrame>,
}

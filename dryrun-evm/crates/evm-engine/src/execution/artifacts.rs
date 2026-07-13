use alloy_primitives::{Bytes, U256};

use crate::{
    EvmExecutionFailure, EvmExecutionStatus, SimulatedBlock, change_observation::Observation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionArtifacts {
    pub(crate) chain_id: u64,
    pub(crate) block: SimulatedBlock,
    pub(crate) status: EvmExecutionStatus,
    pub(crate) gas_used: u64,
    pub(crate) gas_limit: u64,
    pub(crate) fee: U256,
    pub(crate) burnt_fee: U256,
    pub(crate) output: Bytes,
    pub(crate) failure: Option<EvmExecutionFailure>,
    pub(crate) observations: Vec<Observation>,
}

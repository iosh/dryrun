use alloy_primitives::Bytes;

use crate::{
    EvmExecutionFailure, EvmExecutionStatus, SimulatedBlock, change_observation::Observation,
};

use super::fee_settlement::TransactionFeeSettlement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionArtifacts {
    pub(crate) chain_id: u64,
    pub(crate) block: SimulatedBlock,
    pub(crate) status: EvmExecutionStatus,
    pub(crate) gas_used: u64,
    pub(crate) gas_limit: u64,
    pub(crate) fee_settlement: Option<TransactionFeeSettlement>,
    pub(crate) output: Bytes,
    pub(crate) failure: Option<EvmExecutionFailure>,
    pub(crate) observations: Vec<Observation>,
}

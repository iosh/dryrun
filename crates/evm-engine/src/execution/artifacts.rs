use alloy_primitives::Bytes;

use crate::{
    EvmExecutionFailure, EvmExecutionStatus, SimulatedBlock, change_observation::Observation,
};

use super::fee_settlement::TransactionFeeSettlement;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExecutionArtifacts {
    pub(super) chain_id: u64,
    pub(super) block: SimulatedBlock,
    pub(super) status: EvmExecutionStatus,
    pub(super) gas_used: u64,
    pub(super) gas_limit: u64,
    pub(super) fee_settlement: Option<TransactionFeeSettlement>,
    pub(super) output: Bytes,
    pub(super) failure: Option<EvmExecutionFailure>,
    pub(super) observations: Vec<Observation>,
}

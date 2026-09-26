use alloy_primitives::B256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmBlockContext {
    pub number: u64,
    pub hash: B256,
}

pub type EvmSimulation = simulation_core::simulation::Simulation<
    EvmBlockContext,
    crate::TypedTransaction,
    crate::TransactionRequest,
    crate::EvmExecutionOutcome,
    crate::EvmTransactionRejection,
    crate::EvmChangeSet,
    crate::EvmAnalysisError,
>;

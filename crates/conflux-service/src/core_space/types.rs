use alloy_primitives::U256;
use conflux_engine as engine;

pub use engine::core_space::{
    CoreSpaceEpochRef, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionStatus, CoreSpaceStateAnchor,
    CoreSpaceStorageChange,
};
pub use simulation_transaction::TransactionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransactionRequest {
    pub transaction: TransactionRequest,
    pub storage_limit: Option<U256>,
    pub epoch_height: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateCoreSpaceTransactionInput {
    pub epoch: CoreSpaceEpochRef,
    pub transaction: CoreSpaceTransactionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateCoreSpaceTransactionOutput {
    pub execution: CoreSpaceExecution,
}

impl From<engine::core_space::CoreSpaceExecution> for SimulateCoreSpaceTransactionOutput {
    fn from(execution: engine::core_space::CoreSpaceExecution) -> Self {
        Self { execution }
    }
}

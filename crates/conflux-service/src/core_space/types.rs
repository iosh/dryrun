use conflux_engine as engine;

pub use crate::ConfluxTransactionRequest;
pub use engine::core_space::{
    CoreSpaceEpochRef, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionStatus, CoreSpaceStateAnchor,
    CoreSpaceStorageChange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransactionRequest {
    pub transaction: ConfluxTransactionRequest,
    pub storage_limit: Option<u64>,
    pub epoch_height: Option<u64>,
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

use conflux_engine as engine;

pub use engine::core_space::{
    AccessListItem, CoreSpaceEpochRef, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionStatus, CoreSpaceStateAnchor,
    CoreSpaceStorageChange, CoreSpaceTransaction, CoreSpaceTransactionVariant,
    SimulateCoreSpaceTransactionInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateCoreSpaceTransactionOutput {
    pub execution: CoreSpaceExecution,
}

impl From<engine::core_space::CoreSpaceExecution> for SimulateCoreSpaceTransactionOutput {
    fn from(execution: engine::core_space::CoreSpaceExecution) -> Self {
        Self { execution }
    }
}

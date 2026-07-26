use conflux_engine as engine;
pub use engine::espace::{
    AccessListItem, EspaceBlockRef, EspaceExecution, EspaceExecutionFailure,
    EspaceExecutionFailureCode, EspaceExecutionStatus, EspaceTransaction, EspaceTransactionVariant,
    SimulateEspaceTransactionInput, SimulatedBlock,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionOutput {
    pub execution: EspaceExecution,
}

impl From<engine::espace::EspaceExecution> for SimulateEspaceTransactionOutput {
    fn from(execution: engine::espace::EspaceExecution) -> Self {
        Self { execution }
    }
}

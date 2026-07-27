use conflux_engine as engine;
pub use engine::espace::{
    EspaceBlockRef, EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceExecutionStatus, SimulatedBlock,
};
pub use simulation_transaction::TransactionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: TransactionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionOutput {
    pub execution: EspaceExecution,
}

impl From<engine::espace::EspaceExecution> for SimulateEspaceTransactionOutput {
    fn from(execution: engine::espace::EspaceExecution) -> Self {
        Self { execution }
    }
}

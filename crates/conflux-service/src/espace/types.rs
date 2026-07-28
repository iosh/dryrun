pub use crate::ConfluxTransactionRequest;
use conflux_engine as engine;
pub use engine::espace::{
    EspaceBlockRef, EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceExecutionStatus, SimulatedBlock,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: ConfluxTransactionRequest,
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

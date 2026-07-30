pub use crate::ConfluxTransactionRequest;
use conflux_engine as engine;
pub use engine::espace::{
    EspaceBlockRef, EspaceExecutedDetails, EspaceExecution, EspaceExecutionFailure,
    EspaceExecutionFailureCode, EspaceExecutionOutcome, EspaceSimulation, SimulatedBlock,
    StandardChange,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: ConfluxTransactionRequest,
}

pub type SimulateEspaceTransactionOutput = EspaceSimulation;

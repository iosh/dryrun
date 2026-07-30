pub use crate::ConfluxTransactionRequest;
use conflux_engine as engine;
pub use engine::espace::{
    Change, Erc20Metadata, Erc721CollectionMetadata, EspaceBlockRef, EspaceExecutedDetails,
    EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode, EspaceExecutionOutcome,
    EspaceSimulation, NativeMetadata, SimulatedBlock,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: ConfluxTransactionRequest,
}

pub type SimulateEspaceTransactionOutput = EspaceSimulation;

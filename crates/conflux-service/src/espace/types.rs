use conflux_simulation as simulation;
pub use simulation::espace::{
    Change, Erc20Metadata, Erc721CollectionMetadata, EspaceBlockContext, EspaceBlockSelector,
    EspaceExecution, EspaceExecutionDetails, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceOutcome, EspaceSimulation, NativeMetadata,
};
pub use simulation_transaction::TransactionRequest as EspaceTransactionRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulationInput {
    pub block: EspaceBlockSelector,
    pub transaction: EspaceTransactionRequest,
}

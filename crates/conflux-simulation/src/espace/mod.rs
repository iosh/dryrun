mod analysis;
mod changes;
mod execution;
mod outcome;
mod preparer;
mod result;
mod simulator;
mod transaction;

pub use execution::{
    EspaceExecutedDetails, EspaceExecution, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceExecutionOutcome, SimulatedBlock,
};
pub(crate) use outcome::{build_espace_execution, build_espace_not_executed};
pub use preparer::EspaceSimulationPreparer;
pub use result::EspaceSimulation;
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::EspaceSimulator;
pub use transaction::{EspaceBlockRef, EspaceTransaction, EspaceTransactionVariant};
pub(crate) use transaction::{build_espace_transaction_input, validate_espace_transaction};

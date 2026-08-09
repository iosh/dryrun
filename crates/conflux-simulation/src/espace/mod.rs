mod analysis;
mod changes;
mod context;
mod execution;
mod outcome;
mod preparer;
mod result;
mod simulator;
mod transaction;

pub use context::{EspaceBlockContext, EspaceBlockSelector, EspaceContextError};
pub(crate) use context::{ResolvedEspaceContext, resolve_espace_context};
pub use execution::{
    EspaceExecution, EspaceExecutionDetails, EspaceExecutionFailure, EspaceExecutionFailureCode,
    EspaceOutcome,
};
pub(crate) use outcome::{build_espace_execution, build_espace_not_executed};
pub use preparer::EspaceSimulationPreparer;
pub use result::EspaceSimulation;
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::EspaceSimulator;
pub(crate) use transaction::EspaceTransaction;
pub(crate) use transaction::{build_espace_transaction_input, validate_espace_transaction};

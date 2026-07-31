mod execution;
mod outcome;
mod result;
pub(crate) mod simulation;
mod transaction;

pub(crate) use outcome::{build_core_space_execution, build_core_space_not_executed};
pub(crate) use transaction::build_core_space_transaction_input;

pub use contract_standards::StandardChange;
pub use execution::{
    CoreSpaceExecutedDetails, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionOutcome, CoreSpaceStateAnchor,
    CoreSpaceStorageChange,
};
pub use result::CoreSpaceSimulation;
pub use transaction::{CoreSpaceEpochRef, CoreSpaceTransaction, CoreSpaceTransactionVariant};

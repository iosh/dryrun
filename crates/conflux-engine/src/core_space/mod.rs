mod execution;
mod outcome;
pub(crate) mod simulation;
mod transaction;

pub(crate) use outcome::{build_core_space_execution, build_core_space_not_executed};
pub(crate) use transaction::build_core_space_transaction_input;

pub use execution::{
    CoreSpaceExecutedDetails, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionOutcome, CoreSpaceStateAnchor,
    CoreSpaceStorageChange,
};
pub use transaction::{CoreSpaceEpochRef, CoreSpaceTransaction, CoreSpaceTransactionVariant};

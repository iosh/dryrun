mod changes;
mod execution;
mod outcome;
mod result;
pub(crate) mod simulation;
mod transaction;

pub(crate) use outcome::{build_core_space_execution, build_core_space_not_executed};
pub(crate) use transaction::{
    PreparedStoragePayer, build_core_space_transaction_input, prepare_storage_payer,
};

pub use execution::{
    CoreSpaceExecutedDetails, CoreSpaceExecution, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceExecutionOutcome, CoreSpaceStateAnchor,
    CoreSpaceStorageChange,
};
pub use result::CoreSpaceSimulation;
pub use transaction::{CoreSpaceEpochRef, CoreSpaceTransaction, CoreSpaceTransactionVariant};

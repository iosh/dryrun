mod execution;
mod outcome;
mod transaction;

pub(crate) use outcome::{build_core_space_execution, build_core_space_not_executed};
pub(crate) use transaction::build_core_space_transaction_input;

pub use execution::{
    CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
    CoreSpaceExecutionStatus, CoreSpaceStateAnchor, CoreSpaceStorageChange,
};
pub use transaction::{
    AccessListItem, CoreSpaceEpochRef, CoreSpaceTransaction, CoreSpaceTransactionVariant,
    SimulateCoreSpaceTransactionInput,
};

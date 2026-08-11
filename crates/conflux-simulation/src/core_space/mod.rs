mod analysis;
mod changes;
mod completion;
mod context;
mod error;
mod execution;
mod execution_result;
mod outcome;
mod preparer;
mod rejection;
mod result;
mod session;
pub(crate) mod simulation;
mod simulator;
mod transaction;

pub(crate) use completion::complete_transaction;
pub(crate) use outcome::{
    build_core_space_execution, build_core_space_not_executed, convert_executor_outcome,
};
pub(crate) use transaction::{ResolvedStorageSponsorship, resolve_storage_sponsorship};

pub use changes::{
    CoreSpaceChange, CrossSpaceAddress, SponsoredResource, SponsorshipConfiguration,
    SponsorshipEligibilityTarget,
};
pub use conflux_provider::CoreAddress;
pub use context::{
    CoreSpaceBlockContext, CoreSpaceBlockSelector, CoreSpaceContextError, CoreSpaceEpochRef,
    CoreSpaceStateAnchor,
};
pub(crate) use context::{ResolvedCoreSpaceContext, resolve_core_space_context};
pub use error::{
    CoreSpaceExecutionError, CoreSpaceResultIntegrationError, CoreSpaceStateAccessError,
};
pub use execution::{
    CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionOutcome, CoreSpaceLog,
    CoreSpaceLogAddress, CoreSpaceRevertReason, CoreSpaceSuccessOutput,
};
pub use execution_result::{CoreSpaceExecutionResult, CoreSpaceGas};
pub use preparer::CoreSpaceSimulationPreparer;
pub use rejection::CoreSpaceTransactionRejection;
pub use result::CoreSpaceSimulation;
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::CoreSpaceSimulator;
pub use transaction::{
    CoreSpaceAccessListItem, CoreSpaceCompleteTransaction, CoreSpaceCompleteTransactionVariant,
    CoreSpacePartialTransaction, CoreSpacePartialTransactionVariant,
    CoreSpaceTransactionCompletionError, CoreSpaceTransactionInput, CoreSpaceTransactionInputError,
};

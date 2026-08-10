mod analysis;
mod changes;
mod context;
mod execution;
mod outcome;
mod preparer;
mod result;
pub(crate) mod simulation;
mod simulator;
mod transaction;

pub(crate) use outcome::{build_core_space_execution, build_core_space_not_executed};
pub(crate) use transaction::{
    PreparedStoragePayer, build_core_space_transaction_input, prepare_storage_payer,
    validate_core_space_transaction_network,
};

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
pub use execution::{
    CoreSpaceExecution, CoreSpaceExecutionDetails, CoreSpaceExecutionFailure,
    CoreSpaceExecutionFailureCode, CoreSpaceOutcome,
};
pub use preparer::CoreSpaceSimulationPreparer;
pub use result::CoreSpaceSimulation;
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::CoreSpaceSimulator;
pub use transaction::{
    CoreSpaceAccessListItem, CoreSpaceTransactionRequest, CoreSpaceTransactionVariantRequest,
};
pub(crate) use transaction::{CoreSpaceTransaction, CoreSpaceTransactionVariant};

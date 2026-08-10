use conflux_simulation as simulation;

pub use simulation::core_space::{
    Change, CoreSpaceBlockContext, CoreSpaceBlockSelector, CoreSpaceChange, CoreSpaceExecution,
    CoreSpaceExecutionDetails, CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode,
    CoreSpaceOutcome, CoreSpaceSimulation, CrossSpaceAddress, Erc20Metadata,
    Erc721CollectionMetadata, NativeMetadata, SponsoredResource, SponsorshipConfiguration,
    SponsorshipEligibilityTarget,
};
pub use simulation::core_space::{
    CoreAddress, CoreSpaceAccessListItem, CoreSpaceTransactionRequest,
    CoreSpaceTransactionVariantRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransactionInput {
    pub transaction: CoreSpaceTransactionRequest,
    pub storage_limit: Option<u64>,
    pub epoch_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulationInput {
    pub epoch: CoreSpaceBlockSelector,
    pub transaction: CoreSpaceTransactionInput,
}

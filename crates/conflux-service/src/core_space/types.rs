use conflux_simulation as simulation;

pub use simulation::core_space::{
    Change, CoreSpaceBlockContext, CoreSpaceBlockSelector, CoreSpaceChange, CoreSpaceExecution,
    CoreSpaceExecutionFailure, CoreSpaceExecutionOutcome, CoreSpaceExecutionResult, CoreSpaceGas,
    CoreSpaceLog, CoreSpaceLogAddress, CoreSpaceRevertReason, CoreSpaceSimulation,
    CoreSpaceSuccessOutput, CoreSpaceTransactionRejection, CrossSpaceAddress, Erc20Metadata,
    Erc721CollectionMetadata, NativeMetadata, SponsoredResource, SponsorshipConfiguration,
    SponsorshipEligibilityTarget,
};
pub use simulation::core_space::{
    CoreAddress, CoreSpaceAccessListItem, CoreSpaceCompleteTransaction,
    CoreSpaceCompleteTransactionVariant, CoreSpacePartialTransaction,
    CoreSpacePartialTransactionVariant, CoreSpaceTransactionInput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceSimulationInput {
    pub epoch: CoreSpaceBlockSelector,
    pub transaction: CoreSpaceTransactionInput,
}

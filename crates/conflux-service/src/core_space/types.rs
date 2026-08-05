use conflux_simulation as simulation;

pub use crate::ConfluxTransactionRequest;
pub use simulation::core_space::{
    Change, CoreSpaceChange, CoreSpaceEpochRef, CoreSpaceExecutedDetails, CoreSpaceExecution,
    CoreSpaceExecutionFailure, CoreSpaceExecutionFailureCode, CoreSpaceExecutionOutcome,
    CoreSpaceSimulation, CoreSpaceStateAnchor, CrossSpaceAddress, Erc20Metadata,
    Erc721CollectionMetadata, NativeMetadata, SponsoredResource, SponsorshipConfiguration,
    SponsorshipEligibilityTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransactionRequest {
    pub transaction: ConfluxTransactionRequest,
    pub storage_limit: Option<u64>,
    pub epoch_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateCoreSpaceTransactionInput {
    pub epoch: CoreSpaceEpochRef,
    pub transaction: CoreSpaceTransactionRequest,
}

pub type SimulateCoreSpaceTransactionOutput = CoreSpaceSimulation;

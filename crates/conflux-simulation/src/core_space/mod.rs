mod analysis;
mod changes;
mod completion;
mod context;
mod error;
mod execution;
mod execution_result;
mod outcome;
mod rejection;
mod request;
mod result;
mod session;
mod simulator;
mod transaction;

pub(crate) use completion::complete_transaction;
pub(crate) use outcome::{
    build_core_space_execution, build_core_space_not_executed, convert_executor_outcome,
};
pub(crate) use transaction::{ResolvedStorageSponsorship, resolve_storage_sponsorship};

pub use changes::{
    CoreSpaceChange, CoreSpaceNativeCurrency, CrossSpaceAddress, GovernanceParameter,
    GovernanceVote, SponsoredResource, SponsorshipAccessRuleScope, SponsorshipFundingTerms,
    SponsorshipReplacement, VoteAllocation,
};
pub use conflux_provider::CoreAddress;
pub use context::{
    CoreSpaceBlockContext, CoreSpaceBlockSelector, CoreSpaceContextError, CoreSpaceEpochRef,
    CoreSpaceStateAnchor,
};
pub(crate) use context::{ResolvedCoreSpaceContext, resolve_core_space_context};
pub use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, StandardChange,
};
pub use error::{
    CoreSpaceChangesError, CoreSpaceExecutionError, CoreSpaceResultIntegrationError,
    CoreSpaceSimulationError, CoreSpaceStateAccessError,
};
pub use execution::{
    CoreSpaceExecution, CoreSpaceExecutionFailure, CoreSpaceExecutionOutcome, CoreSpaceLog,
    CoreSpaceLogAddress, CoreSpaceRevertReason, CoreSpaceSuccessOutput,
};
pub use execution_result::{CoreSpaceExecutionResult, CoreSpaceGas};
pub use rejection::CoreSpaceTransactionRejection;
pub use request::CoreSpaceSimulationRequest;
pub use result::CoreSpaceSimulation;
pub use simulator::CoreSpaceTransactionSimulator;
pub use transaction::{
    CoreSpaceAccessListItem, CoreSpaceCompleteTransaction, CoreSpaceCompleteTransactionVariant,
    CoreSpacePartialTransaction, CoreSpacePartialTransactionVariant,
    CoreSpaceTransactionCompletionError, CoreSpaceTransactionInput, CoreSpaceTransactionInputError,
};

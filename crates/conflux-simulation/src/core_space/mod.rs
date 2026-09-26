mod analysis;
#[cfg(feature = "serde")]
mod change_codec;
mod changes;
mod completion;
mod context;
mod error;
mod executed_transaction;
mod execution;
mod execution_result;
mod outcome;
mod rejection;
mod request;
mod result;
mod session;
mod simulator;
mod state_access;
mod transaction;
#[cfg(feature = "serde")]
mod transaction_codec;

pub(crate) use completion::complete_transaction;
pub(crate) use transaction::{StorageSponsorship, check_storage_sponsorship};

pub use analysis::{CoreSpaceAnalysisDomain, CoreSpaceAnalysisView, CoreSpaceAnalyzerRegistry};
pub use changes::{
    ContractAdminState, CoreSpaceChange, CoreSpaceChangeSet, CoreSpaceChangeSetBuilder,
    CoreSpaceNativeCurrency, CrossSpaceAddress, GovernanceParameter, GovernanceVote,
    SponsoredResource, SponsorshipAccessRuleScope, SponsorshipFundingTerms, SponsorshipReplacement,
    StoragePoints, VoteAllocation,
};

pub use conflux_provider::CoreAddress;
pub use context::{CoreSpaceBlockContext, CoreSpaceBlockSelector, CoreSpaceContextError};
pub(crate) use context::{CoreSpaceContext, prepare_core_space_context};
pub use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, StandardChange,
};
pub use error::{
    CoreSpaceAnalysisError, CoreSpaceExecutionError, CoreSpaceProtocolError,
    CoreSpaceResultIntegrationError, CoreSpaceSimulationError, CoreSpaceStateAccessError,
    CoreSpaceTransactionCompletionError,
};
pub use executed_transaction::{
    CoreSpaceCallKind, CoreSpaceCommittedFrame, CoreSpaceCommittedInternalTransfer,
    CoreSpaceExecutedTransaction, CoreSpaceExecutionPosition, CoreSpaceExecutionSpace,
    CoreSpaceExecutionStatus, CoreSpaceFrameAction, CoreSpaceFrameId, CoreSpaceLogCheckpoint,
    CoreSpaceTransferPocket,
};
pub use execution::{
    CoreSpaceExecutionFailure, CoreSpaceExecutionOutcome, CoreSpaceLog, CoreSpaceLogAddress,
    CoreSpaceRevertReason, CoreSpaceSuccessOutput,
};
pub use execution_result::{CoreSpaceExecutionResult, CoreSpaceGas};
pub use rejection::CoreSpaceTransactionRejection;
pub use request::CoreSpaceSimulationRequest;
pub use result::{CoreSpaceChanges, CoreSpaceSimulation};
pub use simulator::CoreSpaceTransactionSimulator;
pub use state_access::{
    CoreSpaceDepositLot, CoreSpacePoSRegistrationState, CoreSpaceSimulationLimits,
    CoreSpaceStateAccess, CoreSpaceStateReader, CoreSpaceVoteLockInfo,
};
pub use transaction::{
    CoreSpaceAccessListItem, CoreSpacePartialTransactionCommon, CoreSpaceTransactionCommon,
    CoreSpaceTransactionInput, CoreSpaceTransactionInputError, CoreSpaceTransactionRequest,
    CoreSpaceTransactionType, CoreSpaceTypedTransaction,
};

pub use crate::execution::ReadCallOutcome as CoreSpaceReadCallOutcome;

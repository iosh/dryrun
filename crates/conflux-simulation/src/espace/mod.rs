mod analysis;
mod changes;
mod completion;
mod context;
mod error;
mod executed_transaction;
mod execution;
mod execution_result;
mod outcome_mapping;
mod rejection;
mod result;
mod simulator;
mod state_access;
mod transaction;
mod transaction_adapter;

pub use analysis::{EspaceAnalysisDomain, EspaceAnalysisView, EspaceAnalyzerRegistry};
pub use changes::{
    EspaceAccountDelegation, EspaceAccountDelegationChange, EspaceChange, EspaceChangeSet,
    EspaceChanges, EspaceNativeCurrency, EspaceNativeTransferChange, EspaceSelfDestructBurnChange,
    EspaceStandardChange, EspaceStateChange, EspaceWrappedNativeDepositChange,
    EspaceWrappedNativeWithdrawalChange,
};

pub(crate) use completion::complete_transaction;
pub use context::{EspaceBlockContext, EspaceBlockSelector, EspaceContextError};
pub(crate) use context::{EspaceContext, prepare_espace_context};
pub use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, StandardChange,
};
pub use error::{
    EspaceAnalysisError, EspaceExecutionError, EspaceResultIntegrationError, EspaceSimulationError,
    EspaceStateAccessError, EspaceTransactionCompletionError,
};
pub use executed_transaction::{
    EspaceAppliedAuthorization, EspaceCallKind, EspaceCommittedFrame,
    EspaceCommittedInternalTransfer, EspaceCommittedLog, EspaceCommittedStorageWrite,
    EspaceContractAddress, EspaceExecutedTransaction, EspaceExecutionPosition,
    EspaceExecutionSpace, EspaceExecutionStatus, EspaceFrameAction, EspaceFrameId,
    EspaceLogCheckpoint, EspaceStorageChange, EspaceTransferPocket,
};
pub use execution::{
    EspaceExecutionFailure, EspaceExecutionOutcome, EspaceLog, EspaceLogAddress,
    EspaceRevertReason, EspaceSuccessOutput,
};
pub use execution_result::{EspaceExecutionResult, EspaceFee, EspaceGas};
pub(crate) use outcome_mapping::map_executor_outcome;
pub use rejection::EspaceTransactionRejection;
pub use result::EspaceSimulation;
pub use simulator::EspaceTransactionSimulator;
pub use state_access::{
    EspaceAccountState, EspaceReadCallOutcome, EspaceSimulationLimits, EspaceStateAccess,
    EspaceStateReadError, EspaceStateReader,
};
pub use transaction::{
    AccessListItem, Authorization, DynamicFees, EspaceTransactionCommon, EspaceTransactionInput,
    EspaceTransactionInputError, EspaceTransactionRequest, EspaceTypedTransaction, FeeInput,
    PartialTransactionCommon, SignedAuthorization, TxType,
};
pub(crate) use transaction_adapter::build_executor_transaction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulationRequest {
    pub block: EspaceBlockSelector,
    pub transaction: EspaceTransactionInput,
}

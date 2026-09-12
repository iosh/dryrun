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
mod settlement;
mod simulator;
mod state_access;
mod transaction;
mod transaction_adapter;

pub use changes::{
    CombinedEspaceChangeRules, DefaultEspaceChangeRules, EspaceAccountDelegation,
    EspaceAccountDelegationChange, EspaceAccountDelegationChangeRules, EspaceChange,
    EspaceChangeDerivationError, EspaceChangeRules, EspaceChangeSet, EspaceChangeSetBuilder,
    EspaceChanges, EspaceNativeAssetChangeRules, EspaceNativeCurrency, EspaceNativeTransferChange,
    EspaceObservationRequirements, EspaceSelfDestructBurnChange, EspaceStandardChange,
    EspaceStateChange, EspaceWrappedNativeDepositChange, EspaceWrappedNativeWithdrawalChange,
};
pub(crate) use changes::{
    MetadataReadError, NestedEspaceEffects, ReadCallOutcome, execute_read_call,
};
pub(crate) use completion::complete_transaction;
pub use context::{EspaceBlockContext, EspaceBlockSelector, EspaceContextError};
pub(crate) use context::{ResolvedEspaceContext, resolve_espace_context};
pub use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, StandardChange,
};
pub use error::{
    EspaceChangesError, EspaceExecutionError, EspaceResultIntegrationError, EspaceSimulationError,
    EspaceStateAccessError, EspaceTransactionCompletionError,
};
pub use executed_transaction::{
    EspaceAppliedAuthorization, EspaceCallKind, EspaceCommittedFrame,
    EspaceCommittedInternalTransfer, EspaceCommittedLog, EspaceCommittedStorageWrite,
    EspaceContractAddress, EspaceExecutedTransaction, EspaceExecutionPosition,
    EspaceExecutionSpace, EspaceExecutionStatus, EspaceFrameAction, EspaceFrameId,
    EspaceObservationError, EspaceSemanticLogOccurrence, EspaceStorageChange, EspaceTransferPocket,
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
    EspaceAccountState, EspaceOccurrenceHandle, EspaceOccurrenceStateReaders,
    EspaceReadCallOutcome, EspaceSimulationLimits, EspaceStateAccess, EspaceStateReadError,
    EspaceStateReader,
};
pub use transaction::{
    AccessListItem, Authorization, EspaceCompleteTransaction, EspacePartialTransaction,
    EspaceTransactionCommon, EspaceTransactionInput, EspaceTransactionInputError,
    SignedAuthorization, TxType,
};
pub(crate) use transaction_adapter::{
    build_executor_transaction, validate_transaction_for_execution,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulationRequest {
    pub block: EspaceBlockSelector,
    pub transaction: EspaceTransactionInput,
}

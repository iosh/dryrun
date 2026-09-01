use alloy::primitives::B256;

mod chain_spec;
mod changeset;
mod completion;
mod context;
mod error;
mod execution;
mod execution_result;
mod limits;
mod outcome;
mod rejection;
mod simulation;
mod simulator;
mod standard_resolver;
mod state;
mod transaction;

pub(crate) use chain_spec::{EthereumChainSpec, EthereumExecutionSpec};
pub use changeset::{
    CombinedEvmChangeResolver, EvmAccountDelegation, EvmAccountDelegationChange,
    EvmAccountDelegationResolver, EvmChangeResolutionError, EvmChangeResolver, EvmChangeSet,
    EvmChangeSetBuilder, EvmChanges, EvmNativeChangeResolver, EvmNativeCurrency,
    EvmNativeTransferChange, EvmSelfDestructBurnChange, EvmStandardChange, EvmStateChange,
    EvmWrappedNativeDepositChange, EvmWrappedNativeWithdrawalChange, StandardEvmChangeResolver,
};
pub(crate) use completion::complete_transaction;
pub(crate) use context::resolve_block;
pub use error::{
    EvmBlockEnvironmentError, EvmBlockResolutionError, EvmExecutionError, EvmInitializationError,
    EvmNotReadyError, EvmResultIntegrationError, EvmSimulationError, EvmStateAccessError,
    EvmTransactionCompletionError,
};
pub use execution::{
    EvmCallKind, EvmCommittedFrame, EvmCommittedFrameKind, EvmCommittedLog,
    EvmCommittedSelfdestruct, EvmExecutionPosition, EvmFrameId, EvmOccurrenceEvidenceError,
    EvmSemanticLogOccurrence, EvmTransactionExecution,
};
pub(crate) use execution::{
    EvmExecutionObserver, EvmTransactionExecutionResult, EvmTransactionExecutor,
    map_executed_outcome,
};
pub use execution_result::{EvmBlobGasFee, EvmExecutionGasFee, EvmExecutionResult, EvmFee, EvmGas};
pub use limits::EvmSimulationLimits;
pub use outcome::{
    EvmExecutionOutcome, EvmHaltReason, EvmOutOfGasReason, EvmRevertReason, EvmSuccessOutput,
    EvmSuccessReason,
};
pub use rejection::EvmTransactionRejection;
pub use simulation::{EvmBlockContext, EvmSimulation};
pub use simulator::EvmTransactionSimulator;
pub(crate) use standard_resolver::EvmStandardChangeResolver;
pub use state::{
    EvmAccountState, EvmOccurrenceHandle, EvmOccurrenceStateViews, EvmReadCallOutcome,
    EvmStateReadError, EvmStateView, EvmStateViews,
};
pub use transaction::{
    AccessListItem, Authorization, CompleteTransaction, CompleteTransactionVariant,
    PartialTransaction, PartialTransactionVariant, SignedAuthorization, TransactionInput,
    TransactionInputError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmBlockSelector {
    Latest,
    Safe,
    Finalized,
    Number(u64),
    Hash(B256),
}

impl std::fmt::Display for EvmBlockSelector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Latest => formatter.write_str("latest"),
            Self::Safe => formatter.write_str("safe"),
            Self::Finalized => formatter.write_str("finalized"),
            Self::Number(number) => write!(formatter, "number {number}"),
            Self::Hash(hash) => write!(formatter, "hash {hash}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulationRequest {
    pub block: EvmBlockSelector,
    pub transaction: TransactionInput,
}

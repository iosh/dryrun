use alloy::primitives::B256;

#[cfg(feature = "serde")]
mod codec;

mod analysis;
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
mod state;
mod token_changes;
mod transaction;

pub use analysis::{EvmAnalysisDomain, EvmAnalysisView, EvmAnalyzerRegistry};
pub(crate) use chain_spec::{EthereumChainSpec, EthereumExecutionSpec};
pub use changeset::{
    EvmAccountDelegation, EvmAccountDelegationChange, EvmChangeSet, EvmChanges, EvmNativeCurrency,
    EvmNativeTransferChange, EvmSelfDestructBurnChange, EvmStandardChange, EvmStateChange,
    EvmWrappedNativeDepositChange, EvmWrappedNativeWithdrawalChange,
};
pub use simulation_core::analysis::{
    AnalysisReport, AnalysisScope, Analyzer, AnalyzerDescriptor, AnalyzerLayer, ChainScope,
    Deployment, ExecutionSpace, FactKind, RegistryError, SupportEvidence,
};

pub(crate) use completion::complete_transaction;
pub(crate) use context::resolve_block;
pub use error::{
    EvmAnalysisError, EvmBlockEnvironmentError, EvmBlockResolutionError, EvmExecutionError,
    EvmInitializationError, EvmNotReadyError, EvmResultIntegrationError, EvmSimulationError,
    EvmStateAccessError, EvmTransactionCompletionError,
};
pub use execution::{
    EvmCallKind, EvmCommittedFrame, EvmCommittedLog, EvmCommittedSelfdestruct,
    EvmExecutionPosition, EvmFrameAction, EvmFrameId, EvmLogCheckpoint, EvmTransactionExecution,
};
pub(crate) use execution::{
    EvmExecutionObserver, EvmTransactionExecutionResult, EvmTransactionExecutor,
};
pub use execution_result::{EvmBlobGasFee, EvmExecutionGasFee, EvmExecutionResult, EvmFee, EvmGas};
pub use limits::EvmSimulationLimits;
pub use outcome::{
    EvmExecutionOutcome, EvmHaltReason, EvmOutOfGasReason, EvmRevertReason, EvmSuccessOutput,
    EvmSuccessReason,
};
pub use rejection::EvmTransactionRejection;
pub use simulation::{EvmBlockContext, EvmSimulation};
pub use simulation_core::observation::LogFilter;
pub use simulator::EvmTransactionSimulator;
pub use state::{
    EvmAccountState, EvmReadCallOutcome, EvmStateAccess, EvmStateReadError, EvmStateReader,
};
pub use transaction::{
    AccessListItem, Authorization, DynamicFees, FeeInput, PartialTransactionCommon,
    SignedAuthorization, TransactionCommon, TransactionInput, TransactionInputError,
    TransactionRequest, TxType, TypedTransaction,
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

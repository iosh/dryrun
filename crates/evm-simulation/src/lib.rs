use alloy::primitives::B256;

mod chain_spec;
mod changes;
mod completion;
mod context;
mod error;
mod execution;
mod outcome;
mod simulation;
mod simulator;
mod transaction;

pub(crate) use chain_spec::EthereumChainSpec;
pub(crate) use changes::EvmNativeChangeError;
pub(crate) use completion::complete_transaction;
pub(crate) use context::resolve_block;
pub use error::{
    EvmBlockResolutionError, EvmChangesError, EvmInitializationError, EvmSimulationError,
    EvmTransactionCompletionError,
};
pub(crate) use execution::{
    EvmExecutionError, EvmExecutionObservation, EvmExecutionObserver, EvmExecutionOutput,
    EvmFeeSettlement, EvmTransactionExecutor, create_database,
};
pub use simulation::{
    EvmBlockContext, EvmExecution, EvmExecutionDetails, EvmExecutionFailure,
    EvmExecutionFailureCode, EvmOutcome, EvmSimulation,
};
pub use simulation_changes::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};
pub use simulator::EvmTransactionSimulator;
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

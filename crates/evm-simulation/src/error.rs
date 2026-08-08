use alloy::transports::TransportError;
use thiserror::Error;
use tokio::task::JoinError;

use contract_standards::legacy::ContractStandardsError;

use crate::{EvmBlockSelector, TransactionInputError};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmInitializationError {
    #[error("failed to fetch the Ethereum chain id: {source}")]
    ChainIdRequest {
        #[source]
        source: TransportError,
    },

    #[error("Ethereum chain id mismatch: expected {expected}, got {actual}")]
    ChainIdMismatch { expected: u64, actual: u64 },
}

impl EvmInitializationError {
    pub(crate) const fn chain_id_request(source: TransportError) -> Self {
        Self::ChainIdRequest { source }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmBlockResolutionError {
    #[error("failed to resolve block selected by {selector}: {source}")]
    Request {
        selector: EvmBlockSelector,
        #[source]
        source: TransportError,
    },

    #[error("provider did not return the block selected by {selector}")]
    BlockNotFound { selector: EvmBlockSelector },
}

impl EvmBlockResolutionError {
    pub(crate) const fn request(selector: EvmBlockSelector, source: TransportError) -> Self {
        Self::Request { selector, source }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmTransactionCompletionError {
    #[error("failed to fetch the sender nonce at block {block_number}: {source}")]
    NonceLookup {
        block_number: u64,
        #[source]
        source: TransportError,
    },

    #[error("failed to estimate transaction gas at block {block_number}: {source}")]
    GasEstimation {
        block_number: u64,
        #[source]
        source: TransportError,
    },

    #[error("failed to fetch the suggested gas price: {source}")]
    GasPriceSuggestion {
        #[source]
        source: TransportError,
    },

    #[error("failed to fetch the suggested max priority fee per gas: {source}")]
    PriorityFeeSuggestion {
        #[source]
        source: TransportError,
    },

    #[error("failed to fetch the current blob base fee: {source}")]
    BlobBaseFeeLookup {
        #[source]
        source: TransportError,
    },

    #[error("block {block_number} does not provide a base fee for dynamic fee completion")]
    MissingBaseFee { block_number: u64 },

    #[error("calculated max fee per gas exceeds u128::MAX")]
    MaxFeePerGasOverflow,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmChangesError {
    #[error("transaction changes failed: {source}")]
    ContractStandards {
        #[source]
        source: Box<ContractStandardsError>,
    },

    #[error("{details}")]
    Derivation { details: String },
}

impl EvmChangesError {
    pub(crate) fn derivation(details: impl Into<String>) -> Self {
        Self::Derivation {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmSimulationError {
    #[error(transparent)]
    Input(#[from] TransactionInputError),

    #[error(transparent)]
    BlockResolution(#[from] EvmBlockResolutionError),

    #[error(transparent)]
    TransactionCompletion(#[from] EvmTransactionCompletionError),

    #[error("{0}")]
    Unsupported(String),

    #[error("{0}")]
    NotReady(String),

    #[error("{0}")]
    BlockContext(String),

    #[error("{0}")]
    StateAccess(String),

    #[error("{0}")]
    Execution(String),

    #[error("blocking EVM simulation task terminated unexpectedly: {source}")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Changes(#[from] EvmChangesError),

    #[error("{0}")]
    Internal(String),
}

impl EvmSimulationError {
    pub(crate) fn not_ready(details: impl Into<String>) -> Self {
        Self::NotReady(details.into())
    }

    pub(crate) fn block_context(details: impl Into<String>) -> Self {
        Self::BlockContext(details.into())
    }

    pub(crate) fn state_access(details: impl Into<String>) -> Self {
        Self::StateAccess(details.into())
    }

    pub(crate) fn execution(details: impl Into<String>) -> Self {
        Self::Execution(details.into())
    }

    pub(crate) fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }

    pub(crate) fn changes(details: impl Into<String>) -> Self {
        Self::Changes(EvmChangesError::derivation(details))
    }

    pub(crate) fn internal(details: impl Into<String>) -> Self {
        Self::Internal(details.into())
    }
}

impl From<ContractStandardsError> for EvmSimulationError {
    fn from(error: ContractStandardsError) -> Self {
        Self::Changes(EvmChangesError::ContractStandards {
            source: Box::new(error),
        })
    }
}

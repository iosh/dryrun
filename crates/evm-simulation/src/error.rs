use alloy::transports::TransportError;
use revm::database::AlloyDBError;
use thiserror::Error;
use tokio::task::JoinError;

use crate::{EvmBlockSelector, TransactionInputError, chain_spec::EthereumChainSpecError};

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
    #[error(transparent)]
    Input(#[from] TransactionInputError),

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
pub enum EvmBlockEnvironmentError {
    #[error("block {block_number} is missing a base fee required by the active hardfork")]
    MissingBaseFee { block_number: u64 },

    #[error("block {block_number} is missing prevRandao required by the active hardfork")]
    MissingPrevRandao { block_number: u64 },

    #[error("block {block_number} is missing excess blob gas required by the active hardfork")]
    MissingExcessBlobGas { block_number: u64 },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmStateAccessError {
    #[error("provider request failed while reading EVM state: {source}")]
    ProviderRequest {
        #[source]
        source: TransportError,
    },

    #[error("provider did not return block {number} requested by the BLOCKHASH opcode")]
    BlockNotFound { number: u64 },
}

impl From<AlloyDBError> for EvmStateAccessError {
    fn from(error: AlloyDBError) -> Self {
        match error {
            AlloyDBError::Transport(source) => Self::ProviderRequest { source },
            AlloyDBError::BlockNotFound(number) => Self::BlockNotFound { number },
        }
    }
}

#[derive(Debug, Error)]
#[error("EVM execution result could not be integrated: {details}")]
pub struct EvmResultIntegrationError {
    details: String,
}

impl EvmResultIntegrationError {
    pub(crate) fn new(details: impl Into<String>) -> Self {
        Self {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmExecutionError {
    #[error(transparent)]
    BlockEnvironment(#[from] EvmBlockEnvironmentError),

    #[error(transparent)]
    StateAccess(#[from] EvmStateAccessError),

    #[error(transparent)]
    ResultIntegration(#[from] EvmResultIntegrationError),

    #[error("transaction validation returned a result that the simulator could not map: {details}")]
    UnmappedTransactionValidation { details: String },

    #[error("EVM execution engine failed: {details}")]
    EngineFailure { details: String },
}

impl EvmExecutionError {
    pub(crate) fn unmapped_transaction_validation(details: impl Into<String>) -> Self {
        Self::UnmappedTransactionValidation {
            details: details.into(),
        }
    }

    pub(crate) fn engine_failure(details: impl Into<String>) -> Self {
        Self::EngineFailure {
            details: details.into(),
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmNotReadyError {
    #[error("Ethereum hardfork {hardfork} is not supported by the EVM executor")]
    UnsupportedHardfork { hardfork: &'static str },
}

impl From<EthereumChainSpecError> for EvmNotReadyError {
    fn from(error: EthereumChainSpecError) -> Self {
        match error {
            EthereumChainSpecError::UnsupportedHardfork { hardfork } => Self::UnsupportedHardfork {
                hardfork: hardfork.name(),
            },
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
    TransactionCompletion(EvmTransactionCompletionError),

    #[error(transparent)]
    NotReady(#[from] EvmNotReadyError),

    #[error(transparent)]
    Execution(#[from] EvmExecutionError),

    /// No Tokio runtime was active while polling the simulation future.
    #[error("EVM simulation requires an active Tokio runtime")]
    RuntimeUnavailable,

    #[error("blocking EVM simulation task terminated unexpectedly: {source}")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },
}

impl From<EvmTransactionCompletionError> for EvmSimulationError {
    fn from(error: EvmTransactionCompletionError) -> Self {
        match error {
            EvmTransactionCompletionError::Input(input) => Self::Input(input),
            error => Self::TransactionCompletion(error),
        }
    }
}

impl EvmSimulationError {
    pub(crate) fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }
}

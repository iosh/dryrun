use alloy::{primitives::U256, transports::TransportError};
use revm::database::AlloyDBError;
use thiserror::Error;
use tokio::task::JoinError;

use contract_standards::legacy::ContractStandardsError;

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
#[non_exhaustive]
pub enum EvmResultIntegrationError {
    #[error(
        "the execution engine returned gas limit {result_gas_limit}, but the transaction gas limit is {transaction_gas_limit}"
    )]
    GasLimitMismatch {
        transaction_gas_limit: u64,
        result_gas_limit: u64,
    },

    #[error(
        "the execution engine returned inconsistent gas accounting: gas limit {gas_limit}, \
         intrinsic gas {intrinsic_gas}, spent before refund {spent_before_refund}, \
         refund credit {refund_credit}, floor gas {floor_gas}"
    )]
    InvalidGasAccounting {
        gas_limit: u64,
        intrinsic_gas: u64,
        spent_before_refund: u64,
        refund_credit: u64,
        floor_gas: u64,
    },

    #[error("burnt execution fee {burnt_amount} exceeds charged execution fee {charged_amount}")]
    BurntFeeExceedsCharged {
        charged_amount: U256,
        burnt_amount: U256,
    },

    #[error("the execution engine returned create output for a call transaction")]
    CreateOutputForCall,

    #[error("the execution engine returned call output for a contract creation transaction")]
    CallOutputForCreate,

    #[error("successful contract creation did not return the created address")]
    MissingCreateAddress,
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

    #[error("the transaction transition has already been applied")]
    TransitionAlreadyApplied,
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

    #[error("EIP-4844 execution is not ready because blob fee settlement is not implemented")]
    Eip4844,

    #[error(
        "EIP-7702 execution is not ready because authorization state handling is not implemented"
    )]
    Eip7702,
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
pub enum EvmChangesError {
    #[error("transaction changes failed: {source}")]
    ContractStandards {
        #[source]
        source: Box<ContractStandardsError>,
    },

    #[error("state access failed while {operation}: {source}")]
    StateAccess {
        operation: String,
        #[source]
        source: EvmStateAccessError,
    },

    #[error("{details}")]
    Analysis { details: String },
}

impl EvmChangesError {
    pub(crate) fn state_access(operation: impl Into<String>, source: AlloyDBError) -> Self {
        Self::StateAccess {
            operation: operation.into(),
            source: source.into(),
        }
    }

    pub(crate) fn analysis(details: impl Into<String>) -> Self {
        Self::Analysis {
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

    #[error(transparent)]
    NotReady(#[from] EvmNotReadyError),

    #[error(transparent)]
    Execution(#[from] EvmExecutionError),

    #[error("blocking EVM simulation task terminated unexpectedly: {source}")]
    ExecutionTask {
        #[source]
        source: JoinError,
    },

    #[error(transparent)]
    Changes(#[from] EvmChangesError),
}

impl EvmSimulationError {
    pub(crate) fn execution_task(source: JoinError) -> Self {
        Self::ExecutionTask { source }
    }

    pub(crate) fn changes(details: impl Into<String>) -> Self {
        Self::Changes(EvmChangesError::analysis(details))
    }

    pub(crate) fn changes_state_access(operation: impl Into<String>, source: AlloyDBError) -> Self {
        Self::Changes(EvmChangesError::state_access(operation, source))
    }
}

impl From<ContractStandardsError> for EvmSimulationError {
    fn from(error: ContractStandardsError) -> Self {
        Self::Changes(EvmChangesError::ContractStandards {
            source: Box::new(error),
        })
    }
}

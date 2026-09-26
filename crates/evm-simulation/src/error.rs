use alloy::transports::TransportError;
use revm::database::AlloyDBError;
use simulation_core::error::{Diagnostic, ErrorCode as Code, ErrorInfo};
use thiserror::Error;

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

    #[error("block {block_number} does not provide a base fee for dynamic fee completion")]
    MissingBaseFee { block_number: u64 },

    #[error("fixed block {block_number} does not provide blob fee parameters")]
    MissingBlobBaseFee { block_number: u64 },

    #[error("calculated max fee per gas exceeds the EVM u128 range")]
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
    TransactionCompletion(#[from] EvmTransactionCompletionError),

    #[error(transparent)]
    NotReady(#[from] EvmNotReadyError),

    #[error(transparent)]
    Execution(#[from] EvmExecutionError),

    #[error(transparent)]
    Runtime(#[from] simulation_core::simulation::RuntimeError),
}

impl ErrorInfo for EvmExecutionError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::BlockEnvironment(_) => Code::ContextUnavailable.diagnostic(),
            Self::StateAccess(error) => error.diagnostic(),
            Self::ResultIntegration(_) | Self::UnmappedTransactionValidation { .. } => {
                Code::ExecutionIntegrationFailed.diagnostic()
            }
            Self::EngineFailure { .. } => Code::ExecutionEngineFailed.diagnostic(),
        }
    }
}

impl ErrorInfo for EvmSimulationError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Input(error) => error.diagnostic(),
            Self::BlockResolution(EvmBlockResolutionError::BlockNotFound { .. }) => {
                Code::ContextNotFound.diagnostic()
            }
            Self::BlockResolution(EvmBlockResolutionError::Request { .. }) => {
                Code::ProviderRequestFailed.diagnostic()
            }
            Self::TransactionCompletion(error) => error.diagnostic(),
            Self::NotReady(_) => Code::UnsupportedSimulation.diagnostic(),
            Self::Execution(error) => error.diagnostic(),
            Self::Runtime(error) => error.diagnostic(),
        }
    }
}

impl ErrorInfo for EvmTransactionCompletionError {
    fn diagnostic(&self) -> Diagnostic {
        let code = match self {
            // Code 3 identifies execution failure. Other RPC errors may mean
            // unavailable node state and must retain their provider identity.
            Self::GasEstimation {
                source: TransportError::ErrorResp(error),
                ..
            } if error.code == 3 => Code::CompletionFailed,
            Self::NonceLookup { .. }
            | Self::GasEstimation { .. }
            | Self::GasPriceSuggestion { .. }
            | Self::PriorityFeeSuggestion { .. } => Code::ProviderRequestFailed,
            Self::MissingBaseFee { .. } | Self::MissingBlobBaseFee { .. } => {
                Code::ContextUnavailable
            }
            Self::MaxFeePerGasOverflow => Code::CompletionFailed,
        };
        code.diagnostic()
    }
}

impl ErrorInfo for EvmStateAccessError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::ProviderRequest { .. } => Code::ProviderRequestFailed,
            Self::BlockNotFound { .. } => Code::StateUnavailable,
        }
        .diagnostic()
    }
}

impl ErrorInfo for crate::EvmStateReadError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::StateAccess(error) => error.diagnostic(),
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::ReadCallFailed { .. } => Code::Internal.diagnostic(),
        }
    }
}
impl ErrorInfo for crate::EvmAnalysisError {
    fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::LimitExceeded(error) => error.diagnostic(),
            Self::StateRead(error) => error.diagnostic(),
            Self::Unsupported { .. } => Code::AnalysisUnsupported.diagnostic(),
            Self::Conflict { .. } => Code::AnalysisValidationFailed.diagnostic(),
            Self::RuleFailure { source, .. } => source_diagnostic(source.as_ref()),
        }
    }
}
// Extension errors are erased; known wrappers above are handled by their typed boundary.
fn source_diagnostic(mut error: &(dyn std::error::Error + 'static)) -> Diagnostic {
    loop {
        if let Some(error) = error.downcast_ref::<crate::EvmAnalysisError>() {
            return error.diagnostic();
        }
        if let Some(error) = error.downcast_ref::<crate::EvmStateReadError>() {
            return error.diagnostic();
        }
        if let Some(error) = error.downcast_ref::<EvmStateAccessError>() {
            return error.diagnostic();
        }
        if let Some(error) =
            error.downcast_ref::<simulation_core::observation::AnalysisLimitExceeded>()
        {
            return error.diagnostic();
        }
        if error.is::<TransportError>() {
            return Code::ProviderRequestFailed.diagnostic();
        }
        let Some(source) = error.source() else {
            return Code::AnalysisValidationFailed.diagnostic();
        };
        error = source;
    }
}

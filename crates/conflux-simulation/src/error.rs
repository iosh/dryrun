use std::fmt;

use alloy::{primitives::U256, transports::TransportError};
use conflux_provider::ConfluxProviderError;
use contract_standards::legacy::ContractStandardsError;
use thiserror::Error;

use crate::{
    ConfluxRpcError,
    execution::{ExecutionBlockContextError, TransactionExecutionError},
};

/// Chain identity values expected from or observed at the paired Conflux endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConfluxEndpointIdentity {
    /// Core Space chain id.
    pub core_space_chain_id: u64,
    /// eSpace chain id represented by the Core Space endpoint.
    pub core_reported_espace_chain_id: u64,
    /// Core Space network id.
    pub network_id: u64,
    /// Chain id represented by the eSpace endpoint.
    pub espace_endpoint_chain_id: u64,
}

impl ConfluxEndpointIdentity {
    pub(crate) const fn new(
        core_space_chain_id: u64,
        core_reported_espace_chain_id: u64,
        network_id: u64,
        espace_endpoint_chain_id: u64,
    ) -> Self {
        Self {
            core_space_chain_id,
            core_reported_espace_chain_id,
            network_id,
            espace_endpoint_chain_id,
        }
    }
}

impl fmt::Display for ConfluxEndpointIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Core Space chain id {}, Core-reported eSpace chain id {}, network id {}, eSpace endpoint chain id {}",
            self.core_space_chain_id,
            self.core_reported_espace_chain_id,
            self.network_id,
            self.espace_endpoint_chain_id,
        )
    }
}

/// An identity field returned by the Core Space `cfx_getStatus` method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfluxCoreStatusIdentityField {
    /// Core Space chain id.
    ChainId,
    /// eSpace chain id reported by the Core Space endpoint.
    EthereumSpaceChainId,
    /// Core Space network id.
    NetworkId,
}

impl fmt::Display for ConfluxCoreStatusIdentityField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ChainId => "chainId",
            Self::EthereumSpaceChainId => "ethereumSpaceChainId",
            Self::NetworkId => "networkId",
        };
        formatter.write_str(name)
    }
}

/// An error that prevents construction of a verified Conflux simulation backend.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfluxInitializationError {
    /// The Core Space endpoint did not return its status.
    #[error("failed to fetch the Core Space status: {source}")]
    CoreStatusRequest {
        /// The underlying Core Space RPC failure.
        #[source]
        source: ConfluxProviderError,
    },

    /// The eSpace endpoint did not return its chain id.
    #[error("failed to fetch the eSpace chain id: {source}")]
    EspaceChainIdRequest {
        /// The underlying eSpace transport failure.
        #[source]
        source: TransportError,
    },

    /// A Core Space identity value cannot be represented by the backend.
    #[error("Core Space status identity field {field} exceeds u64: {actual}")]
    CoreStatusIdentityValueOutOfRange {
        /// The field containing the invalid value.
        field: ConfluxCoreStatusIdentityField,
        /// The value returned by the endpoint.
        actual: U256,
    },

    /// The paired endpoints do not both identify as Conflux mainnet.
    #[error("Conflux endpoint identity mismatch: expected [{expected}], got [{actual}]")]
    EndpointIdentityMismatch {
        /// The identity required by this named constructor.
        expected: ConfluxEndpointIdentity,
        /// The identity assembled from both endpoints.
        actual: ConfluxEndpointIdentity,
    },
}

#[derive(Debug, Error)]
pub enum ConfluxSimulationError {
    #[error("block not found: {block}")]
    BlockNotFound { block: String },

    #[error(transparent)]
    BlockContext(#[from] ExecutionBlockContextError),

    #[error("block context error: {message}")]
    InvalidBlockContext { message: String },

    #[error("state anchor is inconsistent")]
    StateAnchorInconsistent,

    #[error("transaction completion failed: {message}")]
    TransactionCompletion { message: String },

    #[error(transparent)]
    Provider(#[from] ConfluxRpcError),

    #[error("state access failed: {message}")]
    StateAccess { message: String },

    #[error("change analysis failed: {message}")]
    Analysis { message: String },

    #[error("simulation execution failed: {message}")]
    ExecutionInternal { message: String },
}

impl ConfluxSimulationError {
    pub(crate) fn transaction_completion_failed(message: impl Into<String>) -> Self {
        Self::TransactionCompletion {
            message: message.into(),
        }
    }

    pub(crate) fn analysis_failed(message: impl Into<String>) -> Self {
        Self::Analysis {
            message: message.into(),
        }
    }
}

impl From<ContractStandardsError> for ConfluxSimulationError {
    fn from(error: ContractStandardsError) -> Self {
        Self::analysis_failed(error.to_string())
    }
}

impl From<TransactionExecutionError> for ConfluxSimulationError {
    fn from(error: TransactionExecutionError) -> Self {
        match error {
            TransactionExecutionError::BlockContext(error) => Self::BlockContext(error),
            TransactionExecutionError::StateAccess(error) => Self::StateAccess {
                message: error.to_string(),
            },
            error => Self::ExecutionInternal {
                message: error.to_string(),
            },
        }
    }
}

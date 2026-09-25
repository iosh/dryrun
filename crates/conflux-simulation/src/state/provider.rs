use std::future::Future;

use alloy::{
    eips::BlockId,
    network::Ethereum,
    providers::{DynProvider, layers::BlockIdProvider},
};
use cfx_types::Address;
use conflux_provider::{
    AddressError, ConfluxProvider, ConfluxProviderError, CoreAddress, Network as ProviderNetwork,
};
use thiserror::Error;

mod block;
mod state;
mod transaction;

#[derive(Clone)]
pub(crate) struct ConfluxSimulationProvider {
    core_space_address_network: ProviderNetwork,
    pub(crate) espace_provider: DynProvider<Ethereum>,
    pub(crate) core_space_provider: ConfluxProvider,
}

impl ConfluxSimulationProvider {
    pub(crate) fn new(
        espace_provider: DynProvider<Ethereum>,
        core_space_provider: ConfluxProvider,
        core_space_address_network: ProviderNetwork,
    ) -> Self {
        Self {
            core_space_address_network,
            espace_provider,
            core_space_provider,
        }
    }

    pub(crate) fn espace_provider_at(
        &self,
        block: BlockId,
    ) -> BlockIdProvider<DynProvider<Ethereum>> {
        BlockIdProvider::new(self.espace_provider.clone(), block)
    }

    pub(crate) fn core_address(&self, address: Address) -> Result<CoreAddress, ConfluxRpcError> {
        CoreAddress::from_bytes(address.0, self.core_space_address_network)
            .map_err(|source| ConfluxRpcError::AddressEncoding { source })
    }

    pub(crate) fn convert_provider_error(
        operation: &'static str,
        source: ConfluxProviderError,
    ) -> ConfluxRpcError {
        ConfluxRpcError::Core { operation, source }
    }

    pub(crate) async fn core_request<Response, Request>(
        method: &'static str,
        request: Request,
    ) -> Result<Response, ConfluxRpcError>
    where
        Request: Future<Output = Result<Response, ConfluxProviderError>>,
    {
        request
            .await
            .map_err(|error| Self::convert_provider_error(method, error))
    }

    pub(crate) fn alloy_u256_to_u64(
        value: alloy_primitives::U256,
        operation: &'static str,
        field: &'static str,
    ) -> Result<u64, ConfluxRpcError> {
        u64::try_from(value).map_err(|_| ConfluxRpcError::InvalidResponse {
            operation,
            reason: format!("response field {field} exceeds u64"),
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfluxRpcError {
    #[error("failed to encode a Core Space request address: {source}")]
    AddressEncoding {
        #[source]
        source: AddressError,
    },
    #[error("Core Space RPC {operation} failed: {source}")]
    Core {
        operation: &'static str,
        #[source]
        source: ConfluxProviderError,
    },
    #[error("eSpace RPC {operation} failed: {source}")]
    Espace {
        operation: &'static str,
        #[source]
        source: alloy::transports::TransportError,
    },
    #[error("invalid RPC data for {operation}: {reason}")]
    InvalidResponse {
        operation: &'static str,
        reason: String,
    },
}

impl ConfluxRpcError {
    pub fn operation(&self) -> &'static str {
        match self {
            Self::AddressEncoding { .. } => "encode Core Space request address",
            Self::Core { operation, .. }
            | Self::Espace { operation, .. }
            | Self::InvalidResponse { operation, .. } => operation,
        }
    }
}

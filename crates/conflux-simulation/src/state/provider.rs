use std::{future::Future, sync::Arc};

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    providers::{RootProvider, layers::BlockIdProvider},
};
use cfx_addr::Network as RpcNetwork;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::Address;
use conflux_provider::{
    ConfluxProvider, ConfluxProviderError, CoreAddress, EpochNumber as ProviderEpochNumber,
    Network as ProviderNetwork,
};
use thiserror::Error;

mod block;
mod state;
mod transaction;

pub struct ConfluxSimulationProvider {
    core_space_address_network: ProviderNetwork,
    pub(crate) espace_provider: RootProvider,
    pub(crate) core_space_provider: Arc<ConfluxProvider>,
}

impl ConfluxSimulationProvider {
    pub fn new(
        espace_provider: RootProvider,
        core_space_provider: Arc<ConfluxProvider>,
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
        block: cfx_rpc_eth_types::BlockId,
    ) -> Result<BlockIdProvider<RootProvider>, ConfluxRpcError> {
        Ok(BlockIdProvider::new(
            self.espace_provider.clone(),
            Self::alloy_block_id(block)?,
        ))
    }

    pub(crate) fn alloy_block_id(
        block: cfx_rpc_eth_types::BlockId,
    ) -> Result<BlockId, ConfluxRpcError> {
        Ok(BlockId::Number(Self::alloy_block_number(block)?))
    }

    pub(crate) fn alloy_block_number(
        block: cfx_rpc_eth_types::BlockId,
    ) -> Result<BlockNumberOrTag, ConfluxRpcError> {
        match block {
            cfx_rpc_eth_types::BlockId::Latest => Ok(BlockNumberOrTag::Latest),
            cfx_rpc_eth_types::BlockId::Num(number) => {
                let number = u64::try_from(number).map_err(|_| ConfluxRpcError {
                    operation: "convert eSpace block selector",
                    reason: format!("block number exceeds u64: {number}"),
                })?;
                Ok(BlockNumberOrTag::Number(number))
            }
            unsupported => Err(ConfluxRpcError {
                operation: "convert eSpace block selector",
                reason: format!("unsupported block selector: {unsupported:?}"),
            }),
        }
    }

    pub(crate) fn core_address(&self, address: Address) -> Result<CoreAddress, ConfluxRpcError> {
        let mut bytes = [0_u8; 20];
        bytes.copy_from_slice(address.as_bytes());
        CoreAddress::from_bytes(bytes, self.provider_network()).map_err(|error| ConfluxRpcError {
            operation: "encode Core Space RPC address",
            reason: error.to_string(),
        })
    }

    pub(crate) fn provider_network(&self) -> ProviderNetwork {
        self.core_space_address_network
    }

    pub(crate) fn provider_epoch(
        epoch: cfx_rpc_cfx_types::EpochNumber,
    ) -> Result<ProviderEpochNumber, ConfluxRpcError> {
        match epoch {
            cfx_rpc_cfx_types::EpochNumber::Num(number) => {
                Ok(ProviderEpochNumber::Number(number.as_u64()))
            }
            cfx_rpc_cfx_types::EpochNumber::LatestState => Ok(ProviderEpochNumber::LatestState),
            unsupported => Err(ConfluxRpcError {
                operation: "convert Core Space epoch selector",
                reason: format!("unsupported epoch selector: {unsupported:?}"),
            }),
        }
    }

    pub(crate) fn provider_address_to_rpc(
        address: CoreAddress,
    ) -> Result<RpcAddress, ConfluxRpcError> {
        let network = match address.network() {
            ProviderNetwork::Main => RpcNetwork::Main,
            ProviderNetwork::Test => RpcNetwork::Test,
            ProviderNetwork::Id(id) => RpcNetwork::Id(id),
        };
        RpcAddress::try_from_h160(Address::from_slice(&address.bytes()), network).map_err(
            |reason| ConfluxRpcError {
                operation: "decode Core Space RPC address",
                reason,
            },
        )
    }

    pub(crate) fn convert_provider_error(
        method: &'static str,
        error: ConfluxProviderError,
    ) -> ConfluxRpcError {
        ConfluxRpcError {
            operation: method,
            reason: error.to_string(),
        }
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
        u64::try_from(value).map_err(|_| ConfluxRpcError {
            operation,
            reason: format!("response field {field} exceeds u64"),
        })
    }
}

#[derive(Debug, Error)]
#[error("Conflux RPC failed: operation={operation}, reason={reason}")]
pub struct ConfluxRpcError {
    pub(crate) operation: &'static str,
    pub(crate) reason: String,
}

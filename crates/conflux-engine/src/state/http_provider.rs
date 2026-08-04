use std::{future::Future, sync::Arc, time::Instant};

use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::Address;
use conflux_provider::{
    ConfluxProvider, ConfluxProviderError, CoreAddress, EpochNumber as ProviderEpochNumber,
    Network as ProviderNetwork,
};
use jsonrpsee::{
    core::{
        client::{BatchEntry, BatchResponse, ClientT},
        params::BatchRequestBuilder,
        traits::ToRpcParams,
    },
    http_client::{HttpClient, HttpClientBuilder},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

mod block;
mod state;
mod transaction;

pub use transaction::CoreSpaceResourceEstimate;

pub struct HttpConfluxProvider {
    core_space_address_network: Network,
    espace_client: HttpClient,
    pub(crate) core_space_provider: Arc<ConfluxProvider>,
}

impl HttpConfluxProvider {
    pub fn new(
        espace_url: &str,
        core_space_provider: Arc<ConfluxProvider>,
        core_space_address_network: Network,
    ) -> Result<Self, ConfluxRpcError> {
        let espace_client = HttpClientBuilder::default()
            .build(espace_url)
            .map_err(|error| ConfluxRpcError {
                operation: "create eSpace RPC client",
                reason: format!("invalid rpc url or http client config: {error}"),
            })?;

        Ok(Self {
            core_space_address_network,
            espace_client,
            core_space_provider,
        })
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
        match self.core_space_address_network {
            Network::Main => ProviderNetwork::Main,
            Network::Test => ProviderNetwork::Test,
            Network::Id(id) => ProviderNetwork::Id(id),
        }
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
            ProviderNetwork::Main => Network::Main,
            ProviderNetwork::Test => Network::Test,
            ProviderNetwork::Id(id) => Network::Id(id),
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

    async fn espace_rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, ConfluxRpcError>
    where
        R: DeserializeOwned + Send,
        Params: ToRpcParams + Send,
    {
        Self::rpc_request(&self.espace_client, "espace", method, params).await
    }

    async fn rpc_request<R, Params>(
        client: &HttpClient,
        space: &'static str,
        method: &'static str,
        params: Params,
    ) -> Result<R, ConfluxRpcError>
    where
        R: DeserializeOwned + Send,
        Params: ToRpcParams + Send,
    {
        let started_at = Instant::now();
        let result = client.request(method, params).await;

        tracing::debug!(
            rpc_space = space,
            rpc_method = method,
            success = result.is_ok(),
            elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0,
            "remote state RPC request completed"
        );

        result.map_err(|error| ConfluxRpcError {
            operation: method,
            reason: error.to_string(),
        })
    }

    async fn rpc_batch_request<'a>(
        client: &HttpClient,
        space: &'static str,
        batch_name: &'static str,
        batch_size: usize,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, Value>, ConfluxRpcError> {
        let started_at = Instant::now();
        let result = client.batch_request(batch).await;

        tracing::debug!(
            rpc_space = space,
            rpc_batch = batch_name,
            batch_size,
            success = result.is_ok(),
            elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0,
            "remote state RPC batch completed"
        );

        result.map_err(|error| ConfluxRpcError {
            operation: batch_name,
            reason: format!("JSON-RPC batch request failed: {error}"),
        })
    }

    fn insert_batch_request<'a, Params>(
        batch: &mut BatchRequestBuilder<'a>,
        method: &'static str,
        params: Params,
    ) -> Result<(), ConfluxRpcError>
    where
        Params: ToRpcParams,
    {
        batch
            .insert(method, params)
            .map_err(|error| ConfluxRpcError {
                operation: method,
                reason: format!("failed to encode JSON-RPC batch parameters: {error}"),
            })
    }

    fn decode_batch_result<'a, T>(
        entries: &mut impl Iterator<Item = BatchEntry<'a, Value>>,
        method: &'static str,
    ) -> Result<T, ConfluxRpcError>
    where
        T: DeserializeOwned,
    {
        let value = entries
            .next()
            .ok_or_else(|| ConfluxRpcError {
                operation: method,
                reason: "missing response in JSON-RPC batch".to_owned(),
            })?
            .map_err(|error| ConfluxRpcError {
                operation: method,
                reason: format!("request failed in JSON-RPC batch: {error}"),
            })?;
        serde_json::from_value(value).map_err(|error| ConfluxRpcError {
            operation: method,
            reason: format!("failed to decode JSON-RPC response: {error}"),
        })
    }

    fn validate_batch_len(
        batch_name: &'static str,
        expected: usize,
        actual: usize,
    ) -> Result<(), ConfluxRpcError> {
        if actual == expected {
            return Ok(());
        }

        Err(ConfluxRpcError {
            operation: batch_name,
            reason: format!("unexpected batch response length: expected {expected}, got {actual}"),
        })
    }
}

#[derive(Debug, Error)]
#[error("Conflux RPC failed: operation={operation}, reason={reason}")]
pub struct ConfluxRpcError {
    pub(crate) operation: &'static str,
    pub(crate) reason: String,
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use conflux_provider::CoreAddress;

    use super::HttpConfluxProvider;

    #[test]
    fn alloy_u256_to_u64_rejects_overflow() {
        assert_eq!(
            HttpConfluxProvider::alloy_u256_to_u64(U256::from(u64::MAX), "test", "value",).unwrap(),
            u64::MAX
        );
        assert!(
            HttpConfluxProvider::alloy_u256_to_u64(
                U256::from_limbs([u64::MAX, 1, 0, 0]),
                "test",
                "value",
            )
            .is_err()
        );
    }

    #[test]
    fn provider_address_to_rpc_preserves_network() {
        let address =
            CoreAddress::parse("cfxtest:aarc9abycue0hhzgyrr53m6cxedgccrmmy8m50bu1p").unwrap();

        let rpc_address = HttpConfluxProvider::provider_address_to_rpc(address).unwrap();

        assert_eq!(rpc_address.network, cfx_addr::Network::Test);
    }
}

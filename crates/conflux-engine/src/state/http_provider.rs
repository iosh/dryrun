use std::time::Instant;

use cfx_addr::Network;
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::Address;
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

use crate::state::provider::RemoteStateProviderError;

mod block;
mod state;
mod transaction;

pub struct HttpConfluxProvider {
    core_space_address_network: Network,
    espace_client: HttpClient,
    core_space_client: HttpClient,
}

impl HttpConfluxProvider {
    pub fn new(
        espace_url: &str,
        core_space_url: &str,
        core_space_address_network: Network,
    ) -> Result<Self, RemoteStateProviderError> {
        let espace_client = HttpClientBuilder::default()
            .build(espace_url)
            .map_err(|error| RemoteStateProviderError::InvalidEndpoint {
                message: format!("invalid eSpace rpc url or http client config: {error}"),
            })?;

        let core_space_client =
            HttpClientBuilder::default()
                .build(core_space_url)
                .map_err(|error| RemoteStateProviderError::InvalidEndpoint {
                    message: format!("invalid Core Space rpc url or http client config: {error}"),
                })?;

        Ok(Self {
            core_space_address_network,
            espace_client,
            core_space_client,
        })
    }

    fn cfx_rpc_address(&self, address: Address) -> Result<RpcAddress, RemoteStateProviderError> {
        RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|message| RemoteStateProviderError::AddressEncoding { message })
    }

    async fn espace_rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
    where
        R: DeserializeOwned + Send,
        Params: ToRpcParams + Send,
    {
        Self::rpc_request(&self.espace_client, "espace", method, params).await
    }

    async fn core_space_rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
    where
        R: DeserializeOwned + Send,
        Params: ToRpcParams + Send,
    {
        Self::rpc_request(&self.core_space_client, "core_space", method, params).await
    }

    async fn rpc_request<R, Params>(
        client: &HttpClient,
        space: &'static str,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
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

        result.map_err(|error| RemoteStateProviderError::RpcRequest {
            operation: method,
            message: error.to_string(),
        })
    }

    async fn rpc_batch_request<'a>(
        client: &HttpClient,
        space: &'static str,
        batch_name: &'static str,
        batch_size: usize,
        batch: BatchRequestBuilder<'a>,
    ) -> Result<BatchResponse<'a, Value>, RemoteStateProviderError> {
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

        result.map_err(|error| RemoteStateProviderError::RpcRequest {
            operation: batch_name,
            message: format!("JSON-RPC batch request failed: {error}"),
        })
    }

    fn insert_batch_request<'a, Params>(
        batch: &mut BatchRequestBuilder<'a>,
        method: &'static str,
        params: Params,
    ) -> Result<(), RemoteStateProviderError>
    where
        Params: ToRpcParams,
    {
        batch
            .insert(method, params)
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                operation: method,
                message: format!("failed to encode JSON-RPC batch parameters: {error}"),
            })
    }

    fn decode_batch_result<'a, T>(
        entries: &mut impl Iterator<Item = BatchEntry<'a, Value>>,
        method: &'static str,
    ) -> Result<T, RemoteStateProviderError>
    where
        T: DeserializeOwned,
    {
        let value = entries
            .next()
            .ok_or_else(|| RemoteStateProviderError::RpcRequest {
                operation: method,
                message: "missing response in JSON-RPC batch".to_string(),
            })?
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                operation: method,
                message: format!("request failed in JSON-RPC batch: {error}"),
            })?;
        serde_json::from_value(value).map_err(|error| RemoteStateProviderError::RpcDecode {
            field: method,
            message: error.to_string(),
        })
    }

    fn validate_batch_len(
        batch_name: &'static str,
        expected: usize,
        actual: usize,
    ) -> Result<(), RemoteStateProviderError> {
        if actual == expected {
            return Ok(());
        }

        Err(RemoteStateProviderError::RpcRequest {
            operation: batch_name,
            message: format!("unexpected batch response length: expected {expected}, got {actual}"),
        })
    }
}

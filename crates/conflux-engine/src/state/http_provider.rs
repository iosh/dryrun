use std::{sync::Arc, time::Instant};

use async_trait::async_trait;
use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_types::{Address, H256, U256};
use jsonrpsee::{
    core::{
        client::{BatchEntry, BatchResponse, ClientT},
        params::BatchRequestBuilder,
        traits::ToRpcParams,
    },
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use primitives::{DepositInfo, VoteStakeInfo};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::state::{
    provider::{RemoteStateProvider, RemoteStateProviderError},
    rpc_encoding::{RpcStorageWord, decode_rpc_bytes},
    rpc_types::{
        CoreSpaceGlobalSnapshot, CoreSpacePoSEconomics, CoreSpaceRpcAccount, CoreSpaceRpcBlock,
        CoreSpaceSponsorInfo, CoreSpaceStorageCollateralInfo, CoreSpaceSupplyInfo,
        CoreSpaceVoteParamsInfo, EspaceAccountSnapshot, EspaceRpcBlock,
    },
};

pub struct HttpConfluxStateProvider {
    core_space_address_network: Network,
    espace_client: HttpClient,
    core_space_client: HttpClient,
}

impl HttpConfluxStateProvider {
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

#[async_trait]
impl RemoteStateProvider for HttpConfluxStateProvider {
    async fn get_espace_storage_at(
        &self,
        block_number: BlockId,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let value: H256 = self
            .espace_rpc_request(
                "eth_getStorageAt",
                rpc_params![
                    address,
                    U256::from_big_endian(slot.as_bytes()),
                    block_number
                ],
            )
            .await?;

        let value = U256::from_big_endian(value.as_bytes());
        Ok((!value.is_zero()).then_some(value))
    }

    async fn get_espace_account_snapshot(
        &self,
        block_number: BlockId,
        address: Address,
    ) -> Result<EspaceAccountSnapshot, RemoteStateProviderError> {
        const BATCH_NAME: &str = "eSpace account";
        const BATCH_LEN: usize = 3;

        let mut batch = BatchRequestBuilder::new();
        Self::insert_batch_request(
            &mut batch,
            "eth_getBalance",
            rpc_params![address, block_number],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "eth_getTransactionCount",
            rpc_params![address, block_number],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "eth_getCode",
            rpc_params![address, block_number],
        )?;

        let response =
            Self::rpc_batch_request(&self.espace_client, "espace", BATCH_NAME, BATCH_LEN, batch)
                .await?;
        Self::validate_batch_len(BATCH_NAME, BATCH_LEN, response.len())?;
        let mut entries = response.into_iter();
        let balance = Self::decode_batch_result(&mut entries, "eth_getBalance")?;
        let nonce = Self::decode_batch_result(&mut entries, "eth_getTransactionCount")?;
        let code: String = Self::decode_batch_result(&mut entries, "eth_getCode")?;

        Ok(EspaceAccountSnapshot {
            balance,
            nonce,
            code: Arc::new(decode_rpc_bytes(code, "eth_getCode")?),
        })
    }

    async fn get_core_space_global_snapshot(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceGlobalSnapshot, RemoteStateProviderError> {
        const BATCH_NAME: &str = "Core Space globals";
        const BATCH_LEN: usize = 7;

        let mut batch = BatchRequestBuilder::new();
        Self::insert_batch_request(
            &mut batch,
            "cfx_getInterestRate",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getAccumulateInterestRate",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(&mut batch, "cfx_getSupplyInfo", rpc_params![epoch.clone()])?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getCollateralInfo",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getPoSEconomics",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(
            &mut batch,
            "cfx_getParamsFromVote",
            rpc_params![epoch.clone()],
        )?;
        Self::insert_batch_request(&mut batch, "cfx_getFeeBurnt", rpc_params![epoch])?;

        let response = Self::rpc_batch_request(
            &self.core_space_client,
            "core_space",
            BATCH_NAME,
            BATCH_LEN,
            batch,
        )
        .await?;
        Self::validate_batch_len(BATCH_NAME, BATCH_LEN, response.len())?;
        let mut entries = response.into_iter();

        Ok(CoreSpaceGlobalSnapshot {
            interest_rate: Self::decode_batch_result(&mut entries, "cfx_getInterestRate")?,
            accumulate_interest_rate: Self::decode_batch_result(
                &mut entries,
                "cfx_getAccumulateInterestRate",
            )?,
            supply: Self::decode_batch_result::<CoreSpaceSupplyInfo>(
                &mut entries,
                "cfx_getSupplyInfo",
            )?,
            collateral: Self::decode_batch_result::<CoreSpaceStorageCollateralInfo>(
                &mut entries,
                "cfx_getCollateralInfo",
            )?,
            pos_economics: Self::decode_batch_result::<CoreSpacePoSEconomics>(
                &mut entries,
                "cfx_getPoSEconomics",
            )?,
            vote_params: Self::decode_batch_result::<CoreSpaceVoteParamsInfo>(
                &mut entries,
                "cfx_getParamsFromVote",
            )?,
            fee_burnt: Self::decode_batch_result(&mut entries, "cfx_getFeeBurnt")?,
        })
    }

    async fn get_core_space_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceRpcAccount, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.core_space_rpc_request("cfx_getAccount", rpc_params![address, epoch])
            .await
    }

    async fn get_core_space_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.core_space_rpc_request("cfx_getDepositList", rpc_params![address, epoch])
            .await
    }

    async fn get_core_space_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.core_space_rpc_request("cfx_getVoteList", rpc_params![address, epoch])
            .await
    }

    async fn get_core_space_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceSponsorInfo, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.core_space_rpc_request("cfx_getSponsorInfo", rpc_params![address, epoch])
            .await
    }

    async fn get_core_space_code_at(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: String = self
            .core_space_rpc_request("cfx_getCode", rpc_params![address, epoch])
            .await?;

        decode_rpc_bytes(value, "cfx_getCode")
    }

    async fn get_core_space_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let slot = U256::from_big_endian(slot.as_bytes());
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: RpcStorageWord = self
            .core_space_rpc_request("cfx_getStorageAt", rpc_params![address, slot, epoch])
            .await?;

        value.into_option_u256()
    }

    async fn call_core_space(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let to = RpcAddress::try_from_h160(to, self.core_space_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);
        let request = CoreSpaceCallRequest {
            to,
            data: format!("0x{}", hex::encode(data)),
        };

        let value: String = self
            .core_space_rpc_request("cfx_call", rpc_params![request, epoch])
            .await?;
        decode_rpc_bytes(value, "cfx_call")
    }

    async fn get_core_space_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, RemoteStateProviderError> {
        self.core_space_rpc_request(
            "cfx_getBlockByEpochNumber",
            rpc_params![epoch_number, false],
        )
        .await
    }

    async fn get_espace_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBlockByNumber", rpc_params![block_number, false])
            .await
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CoreSpaceCallRequest {
    to: RpcAddress,
    data: String,
}

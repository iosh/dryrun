use std::future::IntoFuture;

use cfx_addr::{EncodingOptions, Network, cfx_addr_encode};
use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{Address, H256, U64, U256};
use jsonrpsee::{
    core::{client::ClientT, traits::ToRpcParams},
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use serde::{Deserialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSupplyInfo {
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_espace_tokens: U256,
    pub total_collateral: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStorageCollateralInfo {
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVoteParamsInfo {
    pub pow_base_reward: U256,
    pub base_fee_share_prop: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EspaceRpcBlock {
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRpcAccount {
    pub balance: U256,
    pub nonce: U256,
    pub staking_balance: U256,
    pub collateral_for_storage: U256,
    pub accumulated_interest_return: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRpcBlock {
    pub hash: H256,
    pub height: U256,
    pub miner: RpcAddress,
    pub block_number: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub timestamp: U256,
}

pub trait RemoteStateProvider: Send + Sync {
    fn get_espace_storage_at(
        &self,
        block_id: &str,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    fn get_espace_code_at(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    fn get_espace_balance(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError>;

    fn get_espace_transaction_count(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError>;

    fn get_native_interest_rate(&self, epoch: &str) -> Result<U256, RemoteStateProviderError>;

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: &str,
    ) -> Result<U256, RemoteStateProviderError>;

    fn get_native_supply_info(
        &self,
        epoch: &str,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError>;

    fn get_native_collateral_info(
        &self,
        epoch: &str,
    ) -> Result<NativeStorageCollateralInfo, RemoteStateProviderError>;

    fn get_native_pos_economics(
        &self,
        epoch: &str,
    ) -> Result<NativePoSEconomics, RemoteStateProviderError>;

    fn get_native_vote_params(
        &self,
        epoch: &str,
    ) -> Result<NativeVoteParamsInfo, RemoteStateProviderError>;

    fn get_native_fee_burnt(&self, epoch: &str) -> Result<U256, RemoteStateProviderError>;

    fn get_native_account(
        &self,
        epoch: &str,
        address: Address,
    ) -> Result<NativeRpcAccount, RemoteStateProviderError>;

    fn get_native_block_by_epoch_number(
        &self,
        epoch_number: &str,
    ) -> Result<Option<NativeRpcBlock>, RemoteStateProviderError>;

    fn get_espace_block_by_number(
        &self,
        block_number: &str,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError>;
}

pub struct HttpEspaceProvider {
    espace_client: HttpClient,
    native_client: HttpClient,
    runtime: HandleOrRuntime,
}

impl HttpEspaceProvider {
    pub fn new(rpc_url: String) -> Result<Self, RemoteStateProviderError> {
        Self::new_with_native_rpc(rpc_url.clone(), rpc_url)
    }

    pub fn new_with_native_rpc(
        espace_rpc_url: String,
        native_rpc_url: String,
    ) -> Result<Self, RemoteStateProviderError> {
        let espace_client = HttpClientBuilder::default()
            .build(&espace_rpc_url)
            .map_err(|error| RemoteStateProviderError::Config {
                message: format!("invalid eSpace rpc url or http client config: {error}"),
            })?;

        let native_client = HttpClientBuilder::default()
            .build(&native_rpc_url)
            .map_err(|error| RemoteStateProviderError::Config {
                message: format!("invalid native rpc url or http client config: {error}"),
            })?;

        let runtime = HandleOrRuntime::capture()?;

        Ok(Self {
            espace_client,
            native_client,
            runtime,
        })
    }

    fn espace_rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        self.runtime
            .block_on(self.espace_client.request(method, params))
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                message: error.to_string(),
            })
    }

    fn native_rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        self.runtime
            .block_on(self.native_client.request(method, params))
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                message: error.to_string(),
            })
    }
}

impl RemoteStateProvider for HttpEspaceProvider {
    fn get_espace_storage_at(
        &self,
        block_id: &str,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let value: String = self.espace_rpc_request(
            "eth_getStorageAt",
            rpc_params![
                hex_address(address),
                hex_storage_slot_position(slot),
                block_id
            ],
        )?;

        let value = parse_hex_u256(value, "eth_getStorageAt")?;
        Ok((!value.is_zero()).then_some(value))
    }

    fn get_espace_code_at(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let value: String =
            self.espace_rpc_request("eth_getCode", rpc_params![hex_address(address), block_id])?;

        decode_hex_bytes(value, "eth_getCode")
    }

    fn get_espace_balance(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        let value: String = self.espace_rpc_request(
            "eth_getBalance",
            rpc_params![hex_address(address), block_id],
        )?;

        parse_hex_u256(value, "eth_getBalance")
    }

    fn get_espace_transaction_count(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        let value: String = self.espace_rpc_request(
            "eth_getTransactionCount",
            rpc_params![hex_address(address), block_id],
        )?;

        parse_hex_u256(value, "eth_getTransactionCount")
    }

    fn get_native_interest_rate(&self, epoch: &str) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getInterestRate", rpc_params![epoch])
    }

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: &str,
    ) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getAccumulateInterestRate", rpc_params![epoch])
    }

    fn get_native_supply_info(
        &self,
        epoch: &str,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getSupplyInfo", rpc_params![epoch])
    }

    fn get_native_collateral_info(
        &self,
        epoch: &str,
    ) -> Result<NativeStorageCollateralInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getCollateralInfo", rpc_params![epoch])
    }

    fn get_native_pos_economics(
        &self,
        epoch: &str,
    ) -> Result<NativePoSEconomics, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getPoSEconomics", rpc_params![epoch])
    }

    fn get_native_vote_params(
        &self,
        epoch: &str,
    ) -> Result<NativeVoteParamsInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getParamsFromVote", rpc_params![epoch])
    }

    fn get_native_fee_burnt(&self, epoch: &str) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getFeeBurnt", rpc_params![epoch])
    }

    fn get_native_account(
        &self,
        epoch: &str,
        address: Address,
    ) -> Result<NativeRpcAccount, RemoteStateProviderError> {
        let address = native_mainnet_rpc_address(address)?;

        self.native_rpc_request("cfx_getAccount", rpc_params![address, epoch])
    }

    fn get_native_block_by_epoch_number(
        &self,
        epoch_number: &str,
    ) -> Result<Option<NativeRpcBlock>, RemoteStateProviderError> {
        self.native_rpc_request(
            "cfx_getBlockByEpochNumber",
            rpc_params![epoch_number, false],
        )
    }

    fn get_espace_block_by_number(
        &self,
        block_number: &str,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBlockByNumber", rpc_params![block_number, false])
    }
}

#[derive(Debug)]
enum HandleOrRuntime {
    Handle(Handle),
    Runtime(Runtime),
}

impl HandleOrRuntime {
    fn capture() -> Result<Self, RemoteStateProviderError> {
        match Handle::try_current() {
            Ok(handle) => Ok(Self::Handle(handle)),
            Err(_) => Runtime::new().map(Self::Runtime).map_err(|error| {
                RemoteStateProviderError::Runtime {
                    message: format!("failed to create tokio runtime: {error}"),
                }
            }),
        }
    }

    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: IntoFuture,
    {
        let future = future.into_future();

        match self {
            Self::Handle(handle) => {
                if can_block_in_place_on_current_runtime() {
                    tokio::task::block_in_place(move || handle.block_on(future))
                } else {
                    handle.block_on(future)
                }
            }
            Self::Runtime(runtime) => runtime.block_on(future),
        }
    }
}

fn can_block_in_place_on_current_runtime() -> bool {
    Handle::try_current().ok().is_some_and(|handle| {
        !matches!(
            handle.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::CurrentThread
        )
    })
}

fn hex_address(address: Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

fn hex_storage_slot_position(slot: H256) -> String {
    let digits = hex::encode(slot.as_bytes());
    let digits = digits.trim_start_matches('0');

    if digits.is_empty() {
        "0x0".to_owned()
    } else {
        format!("0x{digits}")
    }
}

fn native_mainnet_rpc_address(address: Address) -> Result<String, RemoteStateProviderError> {
    cfx_addr_encode(address.as_bytes(), Network::Main, EncodingOptions::QrCode).map_err(|error| {
        RemoteStateProviderError::Address {
            message: error.to_string(),
        }
    })
}

fn parse_hex_u256(value: String, field: &'static str) -> Result<U256, RemoteStateProviderError> {
    let digits = value.strip_prefix("0x").unwrap_or(&value).to_owned();

    if digits.is_empty() {
        return Ok(U256::zero());
    }

    let bytes = hex::decode(if digits.len() % 2 == 0 {
        digits.clone()
    } else {
        format!("0{digits}")
    })
    .map_err(|error| RemoteStateProviderError::HexValue {
        field,
        value: value.clone(),
        message: error.to_string(),
    })?;

    Ok(U256::from_big_endian(&bytes))
}

fn decode_hex_bytes(
    value: String,
    field: &'static str,
) -> Result<Vec<u8>, RemoteStateProviderError> {
    let digits = value.strip_prefix("0x").unwrap_or(&value).to_owned();

    if digits.is_empty() {
        return Ok(Vec::new());
    }

    hex::decode(&digits).map_err(|error| RemoteStateProviderError::HexValue {
        field,
        value,
        message: error.to_string(),
    })
}

#[derive(Debug, Error)]
pub enum RemoteStateProviderError {
    #[error("remote state provider config error: {message}")]
    Config { message: String },

    #[error("remote state provider runtime error: {message}")]
    Runtime { message: String },

    #[error("remote state rpc request failed: {message}")]
    RpcRequest { message: String },

    #[error(
        "remote state rpc hex value decode failed: field={field},
      value={value}, reason={message}"
    )]
    HexValue {
        field: &'static str,
        value: String,
        message: String,
    },

    #[error("remote state rpc address encode failed: {message}")]
    Address { message: String },
}

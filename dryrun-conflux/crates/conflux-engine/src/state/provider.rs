use std::future::IntoFuture;

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
}

pub struct HttpEspaceProvider {
    client: HttpClient,
    runtime: HandleOrRuntime,
}

impl HttpEspaceProvider {
    pub fn new(rpc_url: String) -> Result<Self, RemoteStateProviderError> {
        let client = HttpClientBuilder::default()
            .build(&rpc_url)
            .map_err(|error| RemoteStateProviderError::Config {
                message: format!("invalid rpc url or http client config: {error}"),
            })?;

        let runtime = HandleOrRuntime::capture()?;

        Ok(Self { client, runtime })
    }

    fn rpc_request<R, Params>(
        &self,
        method: &'static str,
        params: Params,
    ) -> Result<R, RemoteStateProviderError>
    where
        R: DeserializeOwned,
        Params: ToRpcParams + Send,
    {
        self.runtime
            .block_on(self.client.request(method, params))
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
        let value: String = self.rpc_request(
            "eth_getStorageAt",
            rpc_params![hex_address(address), hex_h256(slot), block_id],
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
            self.rpc_request("eth_getCode", rpc_params![hex_address(address), block_id])?;

        decode_hex_bytes(value, "eth_getCode")
    }

    fn get_espace_balance(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        let value: String = self.rpc_request(
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
        let value: String = self.rpc_request(
            "eth_getTransactionCount",
            rpc_params![hex_address(address), block_id],
        )?;

        parse_hex_u256(value, "eth_getTransactionCount")
    }

    fn get_native_interest_rate(&self, epoch: &str) -> Result<U256, RemoteStateProviderError> {
        self.rpc_request("cfx_getInterestRate", rpc_params![epoch])
    }

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: &str,
    ) -> Result<U256, RemoteStateProviderError> {
        self.rpc_request("cfx_getAccumulateInterestRate", rpc_params![epoch])
    }

    fn get_native_supply_info(
        &self,
        epoch: &str,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError> {
        self.rpc_request("cfx_getSupplyInfo", rpc_params![epoch])
    }

    fn get_native_collateral_info(
        &self,
        epoch: &str,
    ) -> Result<NativeStorageCollateralInfo, RemoteStateProviderError> {
        self.rpc_request("cfx_getCollateralInfo", rpc_params![epoch])
    }

    fn get_native_pos_economics(
        &self,
        epoch: &str,
    ) -> Result<NativePoSEconomics, RemoteStateProviderError> {
        self.rpc_request("cfx_getPoSEconomics", rpc_params![epoch])
    }

    fn get_native_vote_params(
        &self,
        epoch: &str,
    ) -> Result<NativeVoteParamsInfo, RemoteStateProviderError> {
        self.rpc_request("cfx_getParamsFromVote", rpc_params![epoch])
    }

    fn get_native_fee_burnt(&self, epoch: &str) -> Result<U256, RemoteStateProviderError> {
        self.rpc_request("cfx_getFeeBurnt", rpc_params![epoch])
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

fn hex_h256(value: H256) -> String {
    format!("0x{}", hex::encode(value.as_bytes()))
}

fn parse_hex_u256(value: String, field: &'static str) -> Result<U256, RemoteStateProviderError> {
    let digits = value.strip_prefix("0x").unwrap_or(&value).to_owned();

    if digits.is_empty() {
        return Ok(U256::zero());
    }

    U256::from_str_radix(&digits, 16).map_err(|error| RemoteStateProviderError::HexValue {
        field,
        value,
        message: error.to_string(),
    })
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
}

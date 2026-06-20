use std::future::IntoFuture;

use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{Address, H256, U256};
use jsonrpsee::{
    core::{client::ClientT, traits::ToRpcParams},
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use serde::de::DeserializeOwned;
use tokio::runtime::{Handle, Runtime};

use crate::{
    config::ConfluxConfig,
    state::{
        provider::{RemoteStateProvider, RemoteStateProviderError},
        rpc_encoding::decode_rpc_bytes,
        rpc_types::{
            EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock,
            NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
        },
    },
};

pub struct HttpConfluxStateProvider {
    config: ConfluxConfig,
    espace_client: HttpClient,
    native_client: HttpClient,
    runtime: HandleOrRuntime,
}

impl HttpConfluxStateProvider {
    pub fn new(config: ConfluxConfig) -> Result<Self, RemoteStateProviderError> {
        let espace_client = HttpClientBuilder::default()
            .build(&config.rpc.evm_url)
            .map_err(|error| RemoteStateProviderError::InvalidEndpoint {
                message: format!("invalid eSpace rpc url or http client config: {error}"),
            })?;

        let native_client = HttpClientBuilder::default()
            .build(&config.rpc.native_url)
            .map_err(|error| RemoteStateProviderError::InvalidEndpoint {
                message: format!("invalid native rpc url or http client config: {error}"),
            })?;

        let runtime = HandleOrRuntime::capture()?;

        Ok(Self {
            config,
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

impl RemoteStateProvider for HttpConfluxStateProvider {
    fn get_espace_storage_at(
        &self,
        block_number: &str,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let value: H256 = self.espace_rpc_request(
            "eth_getStorageAt",
            rpc_params![
                address,
                U256::from_big_endian(slot.as_bytes()),
                block_number
            ],
        )?;

        let value = U256::from_big_endian(value.as_bytes());
        Ok((!value.is_zero()).then_some(value))
    }

    fn get_espace_code_at(
        &self,
        block_number: &str,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let value: String =
            self.espace_rpc_request("eth_getCode", rpc_params![address, block_number])?;

        decode_rpc_bytes(value, "eth_getCode")
    }

    fn get_espace_balance(
        &self,
        block_number: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBalance", rpc_params![address, block_number])
    }

    fn get_espace_transaction_count(
        &self,
        block_number: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request(
            "eth_getTransactionCount",
            rpc_params![address, block_number],
        )
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
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

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
                RemoteStateProviderError::RuntimeInit {
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

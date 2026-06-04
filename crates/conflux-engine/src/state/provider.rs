use alloy::{
    eips::BlockId,
    network::Ethereum,
    primitives::{Address as AlloyAddress, U256 as AlloyU256},
    providers::{DynProvider, Provider, ProviderBuilder},
    transports::http::reqwest,
};

use cfx_types::{Address, H256, U256};
use serde::Deserialize;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeSupplyInfo {
    pub total_issued: U256,
    pub total_staking: U256,
}

/// Remote state access
pub(crate) trait RemoteStateProvider: Send + Sync {
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

    fn get_native_interest_rate(&self, epoch: &str) -> Result<U256, RemoteStateProviderError>;

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: &str,
    ) -> Result<U256, RemoteStateProviderError>;

    fn get_native_supply_info(
        &self,
        epoch: &str,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError>;
}
/// eSpace state reader backed by an alloy provider.
pub(crate) struct HttpEspaceProvider {
    provider: DynProvider<Ethereum>,
    runtime: HandleOrRuntime,
}

impl HttpEspaceProvider {
    pub(crate) fn new(rpc_url: String) -> Result<Self, RemoteStateProviderError> {
        let parsed_url: reqwest::Url =
            rpc_url
                .parse()
                .map_err(|error| RemoteStateProviderError::Config {
                    message: format!("invalid rpc url: {error}"),
                })?;

        let provider = ProviderBuilder::new().connect_http(parsed_url).erased();
        let runtime = HandleOrRuntime::capture()?;

        Ok(Self { provider, runtime })
    }
}

impl RemoteStateProvider for HttpEspaceProvider {
    fn get_espace_storage_at(
        &self,
        block_id: &str,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let alloy_address = AlloyAddress::from_slice(address.as_bytes());
        let alloy_slot = AlloyU256::from_be_slice(slot.as_bytes());
        let block_id = parse_block_id(block_id)?;

        let value = self.runtime.block_on(
            self.provider
                .get_storage_at(alloy_address, alloy_slot)
                .block_id(block_id),
        )?;

        let value = cfx_u256_from_alloy(value);
        Ok((!value.is_zero()).then_some(value))
    }

    fn get_espace_code_at(
        &self,
        block_id: &str,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let alloy_address = AlloyAddress::from_slice(address.as_bytes());
        let block_id = parse_block_id(block_id)?;

        let code = self
            .runtime
            .block_on(self.provider.get_code_at(alloy_address).block_id(block_id))?;

        Ok(code.to_vec())
    }

    fn get_native_interest_rate(&self, epoch: &str) -> Result<U256, RemoteStateProviderError> {
        self.runtime
            .block_on(
                self.provider
                    .client()
                    .request("cfx_getInterestRate", (epoch,)),
            )
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                message: error.to_string(),
            })
    }

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: &str,
    ) -> Result<U256, RemoteStateProviderError> {
        self.runtime
            .block_on(
                self.provider
                    .client()
                    .request("cfx_getAccumulateInterestRate", (epoch,)),
            )
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                message: error.to_string(),
            })
    }

    fn get_native_supply_info(
        &self,
        epoch: &str,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError> {
        self.runtime
            .block_on(
                self.provider
                    .client()
                    .request("cfx_getSupplyInfo", (epoch,)),
            )
            .map_err(|error| RemoteStateProviderError::RpcRequest {
                message: error.to_string(),
            })
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

fn parse_block_id(block_id: &str) -> Result<BlockId, RemoteStateProviderError> {
    block_id
        .parse::<BlockId>()
        .map_err(|error| RemoteStateProviderError::InvalidBlockId {
            block_id: block_id.to_owned(),
            message: error.to_string(),
        })
}

fn cfx_u256_from_alloy(value: AlloyU256) -> U256 {
    U256::from_big_endian(&value.to_be_bytes::<32>())
}

#[derive(Debug, Error)]
pub(crate) enum RemoteStateProviderError {
    #[error("remote state provider config error: {message}")]
    Config { message: String },

    #[error("remote state provider runtime error: {message}")]
    Runtime { message: String },

    #[error("remote state provider invalid block id `{block_id}`: {message}")]
    InvalidBlockId { block_id: String, message: String },

    #[error("remote state rpc request failed: {0}")]
    Rpc(#[from] alloy::transports::TransportError),

    #[error("remote state rpc request failed: {message}")]
    RpcRequest { message: String },
}

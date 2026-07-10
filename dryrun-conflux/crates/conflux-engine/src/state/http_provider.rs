use std::future::IntoFuture;

use cfx_rpc_cfx_types::{EpochNumber, RpcAddress, epoch_number::BlockHashOrEpochNumber};
use cfx_rpc_eth_types::BlockId;
use cfx_types::{Address, H256, U256};
use jsonrpsee::{
    core::{client::ClientT, traits::ToRpcParams},
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use primitives::{DepositInfo, VoteStakeInfo};
use serde::de::DeserializeOwned;
use tokio::runtime::{Handle, Runtime};

use crate::{
    config::ConfluxConfig,
    state::{
        provider::{RemoteStateProvider, RemoteStateProviderError},
        rpc_encoding::{RpcStorageWord, decode_rpc_bytes},
        rpc_types::{
            EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock,
            NativeSponsorInfo, NativeStorageCollateralInfo, NativeSupplyInfo,
            NativeVoteParamsInfo,
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
        block_number: BlockId,
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
        block_number: BlockId,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let value: String =
            self.espace_rpc_request("eth_getCode", rpc_params![address, block_number])?;

        decode_rpc_bytes(value, "eth_getCode")
    }

    fn get_espace_balance(
        &self,
        block_number: BlockId,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBalance", rpc_params![address, block_number])
    }

    fn get_espace_transaction_count(
        &self,
        block_number: BlockId,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError> {
        self.espace_rpc_request(
            "eth_getTransactionCount",
            rpc_params![address, block_number],
        )
    }

    fn get_native_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getInterestRate", rpc_params![epoch])
    }

    fn get_native_accumulate_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getAccumulateInterestRate", rpc_params![epoch])
    }

    fn get_native_supply_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getSupplyInfo", rpc_params![epoch])
    }

    fn get_native_collateral_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeStorageCollateralInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getCollateralInfo", rpc_params![epoch])
    }

    fn get_native_pos_economics(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativePoSEconomics, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getPoSEconomics", rpc_params![epoch])
    }

    fn get_native_vote_params(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeVoteParamsInfo, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getParamsFromVote", rpc_params![epoch])
    }

    fn get_native_fee_burnt(&self, epoch: EpochNumber) -> Result<U256, RemoteStateProviderError> {
        self.native_rpc_request("cfx_getFeeBurnt", rpc_params![epoch])
    }

    fn get_native_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<NativeRpcAccount, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.native_rpc_request("cfx_getAccount", rpc_params![address, epoch])
    }

    fn get_native_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.native_rpc_request("cfx_getDepositList", rpc_params![address, epoch])
    }

    fn get_native_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.native_rpc_request("cfx_getVoteList", rpc_params![address, epoch])
    }

    fn get_native_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<NativeSponsorInfo, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;

        self.native_rpc_request("cfx_getSponsorInfo", rpc_params![address, epoch])
    }

    fn get_native_code_at(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: String =
            self.native_rpc_request("cfx_getCode", rpc_params![address, epoch])?;

        decode_rpc_bytes(value, "cfx_getCode")
    }

    fn get_native_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError> {
        let address = RpcAddress::try_from_h160(address, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let slot = U256::from_big_endian(slot.as_bytes());
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);

        let value: RpcStorageWord =
            self.native_rpc_request("cfx_getStorageAt", rpc_params![address, slot, epoch])?;

        value.into_option_u256()
    }

    fn call_native(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError> {
        let to = RpcAddress::try_from_h160(to, self.config.chain.native_address_network)
            .map_err(|error| RemoteStateProviderError::AddressEncoding { message: error })?;
        let epoch = BlockHashOrEpochNumber::EpochNumber(epoch);
        let request = NativeCallRequest {
            to,
            data: format!("0x{}", hex::encode(data)),
        };

        let value: String = self.native_rpc_request("cfx_call", rpc_params![request, epoch])?;
        decode_rpc_bytes(value, "cfx_call")
    }

    fn get_native_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<NativeRpcBlock>, RemoteStateProviderError> {
        self.native_rpc_request(
            "cfx_getBlockByEpochNumber",
            rpc_params![epoch_number, false],
        )
    }

    fn get_espace_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError> {
        self.espace_rpc_request("eth_getBlockByNumber", rpc_params![block_number, false])
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCallRequest {
    to: RpcAddress,
    data: String,
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

use cfx_types::{Address, H256, U256};
use thiserror::Error;

use crate::state::rpc_types::{
    EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock,
    NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
};

pub trait RemoteStateProvider: Send + Sync {
    fn get_espace_storage_at(
        &self,
        block_number: &str,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    fn get_espace_code_at(
        &self,
        block_number: &str,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    fn get_espace_balance(
        &self,
        block_number: &str,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError>;

    fn get_espace_transaction_count(
        &self,
        block_number: &str,
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

#[derive(Debug, Error)]
pub enum RemoteStateProviderError {
    #[error("remote state provider endpoint error: {message}")]
    InvalidEndpoint { message: String },

    #[error("remote state provider runtime init error: {message}")]
    RuntimeInit { message: String },

    #[error("remote state rpc request failed: {message}")]
    RpcRequest { message: String },

    #[error(
        "remote state rpc decode failed: field={field},
      value={value}, reason={message}"
    )]
    RpcDecode {
        field: &'static str,
        value: String,
        message: String,
    },

    #[error("remote state rpc address encoding failed: {message}")]
    AddressEncoding { message: String },
}

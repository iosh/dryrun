use async_trait::async_trait;
use cfx_types::{Address, H256, U256};
use primitives::{DepositInfo, VoteStakeInfo};
use thiserror::Error;

use crate::state::rpc_types::{
    EspaceAccountSnapshot, EspaceRpcBlock, NativeGlobalSnapshot, NativeRpcAccount, NativeRpcBlock,
    NativeSponsorInfo,
};
use cfx_rpc_cfx_types::EpochNumber;
use cfx_rpc_eth_types::BlockId;

#[async_trait]
pub trait RemoteStateProvider: Send + Sync {
    async fn get_espace_storage_at(
        &self,
        block: BlockId,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    async fn get_espace_account_snapshot(
        &self,
        block: BlockId,
        address: Address,
    ) -> Result<EspaceAccountSnapshot, RemoteStateProviderError>;

    async fn get_native_global_snapshot(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeGlobalSnapshot, RemoteStateProviderError>;

    async fn get_native_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<NativeRpcAccount, RemoteStateProviderError>;

    async fn get_native_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError>;

    async fn get_native_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError>;

    async fn get_native_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<NativeSponsorInfo, RemoteStateProviderError>;

    async fn get_native_code_at(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn get_native_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    async fn call_native(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn get_native_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<NativeRpcBlock>, RemoteStateProviderError>;

    async fn get_espace_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError>;
}

#[derive(Debug, Error)]
pub enum RemoteStateProviderError {
    #[error("remote state provider endpoint error: {message}")]
    InvalidEndpoint { message: String },

    #[error("remote state rpc request failed: {message}")]
    RpcRequest { message: String },

    #[error("remote state rpc decode failed: field={field}, reason={message}")]
    RpcDecode {
        field: &'static str,
        message: String,
    },

    #[error("remote state rpc address encoding failed: {message}")]
    AddressEncoding { message: String },
}

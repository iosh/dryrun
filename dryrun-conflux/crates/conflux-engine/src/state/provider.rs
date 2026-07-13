use async_trait::async_trait;
use cfx_types::{Address, H256, U256};
use primitives::{DepositInfo, VoteStakeInfo};
use thiserror::Error;

use crate::state::rpc_types::{
    EspaceRpcBlock, NativePoSEconomics, NativeRpcAccount, NativeRpcBlock, NativeSponsorInfo,
    NativeStorageCollateralInfo, NativeSupplyInfo, NativeVoteParamsInfo,
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

    async fn get_espace_code_at(
        &self,
        block: BlockId,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn get_espace_balance(
        &self,
        block: BlockId,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn get_espace_transaction_count(
        &self,
        block: BlockId,
        address: Address,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn get_native_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn get_native_accumulate_interest_rate(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn get_native_supply_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeSupplyInfo, RemoteStateProviderError>;

    async fn get_native_collateral_info(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeStorageCollateralInfo, RemoteStateProviderError>;

    async fn get_native_pos_economics(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativePoSEconomics, RemoteStateProviderError>;

    async fn get_native_vote_params(
        &self,
        epoch: EpochNumber,
    ) -> Result<NativeVoteParamsInfo, RemoteStateProviderError>;

    async fn get_native_fee_burnt(
        &self,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError>;

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

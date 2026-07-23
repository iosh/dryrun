use async_trait::async_trait;
use cfx_types::{Address, H256, U256};
use primitives::{DepositInfo, VoteStakeInfo};
use thiserror::Error;

use crate::state::rpc_types::{
    CoreSpaceGlobalSnapshot, CoreSpaceRpcAccount, CoreSpaceRpcBlock, CoreSpaceSponsorInfo,
    EspaceAccountSnapshot, EspaceRpcBlock,
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

    async fn get_core_space_global_snapshot(
        &self,
        epoch: EpochNumber,
    ) -> Result<CoreSpaceGlobalSnapshot, RemoteStateProviderError>;

    async fn get_core_space_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceRpcAccount, RemoteStateProviderError>;

    async fn get_core_space_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError>;

    async fn get_core_space_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError>;

    async fn get_core_space_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceSponsorInfo, RemoteStateProviderError>;

    async fn get_core_space_code_at(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn get_core_space_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    async fn call_core_space(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn get_core_space_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, RemoteStateProviderError>;

    async fn get_espace_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError>;
}

#[derive(Debug, Error)]
pub enum RemoteStateProviderError {
    #[error("remote state provider endpoint error: {message}")]
    InvalidEndpoint { message: String },

    #[error("remote state rpc request failed: operation={operation}, reason={message}")]
    RpcRequest {
        operation: &'static str,
        message: String,
    },

    #[error("remote state rpc decode failed: field={field}, reason={message}")]
    RpcDecode {
        field: &'static str,
        message: String,
    },

    #[error("remote state rpc address encoding failed: {message}")]
    AddressEncoding { message: String },
}

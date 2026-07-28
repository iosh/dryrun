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

use crate::ConfluxTransactionBody;

#[async_trait]
pub trait ConfluxBlockProvider: Send + Sync {
    async fn cfx_get_block_by_epoch_number(
        &self,
        epoch_number: EpochNumber,
    ) -> Result<Option<CoreSpaceRpcBlock>, RemoteStateProviderError>;

    async fn eth_get_block_by_number(
        &self,
        block_number: BlockId,
    ) -> Result<Option<EspaceRpcBlock>, RemoteStateProviderError>;
}

#[async_trait]
pub trait ConfluxStateProvider: Send + Sync {
    async fn eth_get_storage_at(
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

    async fn cfx_get_account(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceRpcAccount, RemoteStateProviderError>;

    async fn cfx_get_deposit_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<DepositInfo>, RemoteStateProviderError>;

    async fn cfx_get_vote_list(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<VoteStakeInfo>, RemoteStateProviderError>;

    async fn cfx_get_sponsor_info(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<CoreSpaceSponsorInfo, RemoteStateProviderError>;

    async fn cfx_get_code(
        &self,
        epoch: EpochNumber,
        address: Address,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;

    async fn cfx_get_storage_at(
        &self,
        epoch: EpochNumber,
        address: Address,
        slot: H256,
    ) -> Result<Option<U256>, RemoteStateProviderError>;

    async fn cfx_call(
        &self,
        epoch: EpochNumber,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, RemoteStateProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpaceResourceEstimate {
    pub gas_limit: U256,
    pub storage_limit: u64,
}

#[async_trait]
pub trait ConfluxTransactionProvider: Send + Sync {
    async fn eth_get_transaction_count(
        &self,
        address: Address,
        block: BlockId,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn eth_gas_price(&self) -> Result<U256, RemoteStateProviderError>;

    async fn eth_max_priority_fee_per_gas(&self) -> Result<U256, RemoteStateProviderError>;

    async fn eth_estimate_gas(
        &self,
        block: BlockId,
        transaction: &ConfluxTransactionBody,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn cfx_get_next_nonce(
        &self,
        address: Address,
        epoch: EpochNumber,
    ) -> Result<U256, RemoteStateProviderError>;

    async fn cfx_gas_price(&self) -> Result<U256, RemoteStateProviderError>;

    async fn cfx_max_priority_fee_per_gas(&self) -> Result<U256, RemoteStateProviderError>;

    async fn cfx_estimate_gas_and_collateral(
        &self,
        epoch: EpochNumber,
        transaction: &ConfluxTransactionBody,
        epoch_height: u64,
        gas_limit: Option<U256>,
        storage_limit: Option<u64>,
    ) -> Result<CoreSpaceResourceEstimate, RemoteStateProviderError>;
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

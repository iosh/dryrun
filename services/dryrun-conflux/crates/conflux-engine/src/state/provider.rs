use cfx_types::{Address, H256, U64, U256};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeSupplyInfo {
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_espace_tokens: U256,
    pub total_collateral: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeStorageCollateralInfo {
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeVoteParamsInfo {
    pub pow_base_reward: U256,
    pub base_fee_share_prop: U256,
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
#[derive(Debug, Error)]
pub(crate) enum RemoteStateProviderError {
    #[error("remote state rpc request failed: {0}")]
    Rpc(String),

    #[error("remote state rpc request failed: {message}")]
    RpcRequest { message: String },
}

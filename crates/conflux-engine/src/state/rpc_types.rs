use std::sync::Arc;

use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{H256, U64, U256};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceSupplyInfo {
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_espace_tokens: U256,
    pub total_collateral: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceStorageCollateralInfo {
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpacePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceVoteParamsInfo {
    pub pow_base_reward: U256,
    pub base_fee_share_prop: U256,
}

#[derive(Debug, Clone)]
pub struct CoreSpaceGlobalSnapshot {
    pub interest_rate: U256,
    pub accumulate_interest_rate: U256,
    pub supply: CoreSpaceSupplyInfo,
    pub collateral: CoreSpaceStorageCollateralInfo,
    pub pos_economics: CoreSpacePoSEconomics,
    pub vote_params: CoreSpaceVoteParamsInfo,
    pub fee_burnt: U256,
}

#[derive(Debug, Clone)]
pub struct EspaceAccountSnapshot {
    pub balance: U256,
    pub nonce: U256,
    pub code: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EspaceRpcBlock {
    pub hash: H256,
    pub number: U256,
    pub base_fee_per_gas: Option<U256>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceRpcAccount {
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: H256,
    pub staking_balance: U256,
    pub collateral_for_storage: U256,
    pub accumulated_interest_return: U256,
    pub admin: RpcAddress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceSponsorInfo {
    pub sponsor_for_gas: RpcAddress,
    pub sponsor_for_collateral: RpcAddress,
    pub sponsor_gas_bound: U256,
    pub sponsor_balance_for_gas: U256,
    pub sponsor_balance_for_collateral: U256,
    pub available_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSpaceRpcBlock {
    pub hash: H256,
    pub height: U256,
    pub miner: RpcAddress,
    pub block_number: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub timestamp: U256,
}

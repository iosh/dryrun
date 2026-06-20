use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{H256, U64, U256};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeSupplyInfo {
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_espace_tokens: U256,
    pub total_collateral: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeStorageCollateralInfo {
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVoteParamsInfo {
    pub pow_base_reward: U256,
    pub base_fee_share_prop: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EspaceRpcBlock {
    pub base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRpcAccount {
    pub balance: U256,
    pub nonce: U256,
    pub staking_balance: U256,
    pub collateral_for_storage: U256,
    pub accumulated_interest_return: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeRpcBlock {
    pub hash: H256,
    pub height: U256,
    pub miner: RpcAddress,
    pub block_number: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub timestamp: U256,
}

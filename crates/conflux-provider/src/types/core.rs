use alloy_primitives::{B256, U256};
use serde::{Deserialize, Serialize};

use crate::CoreAddress;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    pub best_hash: B256,
    pub chain_id: U256,
    pub ethereum_space_chain_id: U256,
    pub network_id: U256,
    pub epoch_number: U256,
    pub block_number: U256,
    pub pending_tx_number: U256,
    pub latest_checkpoint: U256,
    pub latest_confirmed: U256,
    pub latest_state: U256,
    pub latest_finalized: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAndCollateralEstimate {
    pub gas_limit: U256,
    pub gas_used: U256,
    pub storage_collateralized: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSupplyInfo {
    pub total_circulating: U256,
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_collateral: U256,
    pub total_espace_tokens: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreCollateralInfo {
    pub total_storage_tokens: U256,
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreVoteParams {
    pub pow_base_reward: U256,
    pub interest_rate: U256,
    pub storage_point_prop: U256,
    pub base_fee_share_prop: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreAccount {
    pub address: CoreAddress,
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: B256,
    pub staking_balance: U256,
    pub collateral_for_storage: U256,
    pub accumulated_interest_return: U256,
    pub admin: CoreAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSponsorInfo {
    pub sponsor_for_gas: CoreAddress,
    pub sponsor_for_collateral: CoreAddress,
    pub sponsor_gas_bound: U256,
    pub sponsor_balance_for_gas: U256,
    pub sponsor_balance_for_collateral: U256,
    pub available_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositInfo {
    pub amount: U256,
    pub deposit_time: U256,
    pub accumulated_interest_rate: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteStakeInfo {
    pub amount: U256,
    pub unlock_block_number: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRpcBlock {
    pub hash: B256,
    pub epoch_number: Option<U256>,
    pub block_number: Option<U256>,
    pub miner: CoreAddress,
    pub timestamp: U256,
    pub base_fee_per_gas: Option<U256>,
    pub pos_reference: Option<B256>,
}

/// PoS context referenced by a Core pivot.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosBlock {
    pub hash: B256,
    pub height: U256,
    pub pivot_decision: Option<PivotDecision>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PivotDecision {
    pub block_hash: B256,
    pub height: U256,
}

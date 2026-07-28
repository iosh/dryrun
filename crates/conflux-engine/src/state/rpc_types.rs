use std::sync::Arc;

use cfx_rpc_cfx_types::RpcAddress;
use cfx_types::{H256, U64, U256};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceSupplyInfo {
    pub(crate) total_issued: U256,
    pub(crate) total_staking: U256,
    pub(crate) total_espace_tokens: U256,
    pub(crate) total_collateral: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceStorageCollateralInfo {
    pub(crate) converted_storage_points: U256,
    pub(crate) used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpacePoSEconomics {
    pub(crate) total_pos_staking_tokens: U256,
    pub(crate) distributable_pos_interest: U256,
    pub(crate) last_distribute_block: U64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceVoteParamsInfo {
    pub(crate) pow_base_reward: U256,
    pub(crate) base_fee_share_prop: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceGlobals {
    pub(crate) interest_rate: U256,
    pub(crate) accumulate_interest_rate: U256,
    pub(crate) supply: CoreSpaceSupplyInfo,
    pub(crate) collateral: CoreSpaceStorageCollateralInfo,
    pub(crate) pos_economics: CoreSpacePoSEconomics,
    pub(crate) vote_params: CoreSpaceVoteParamsInfo,
    pub(crate) fee_burnt: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct EspaceAccountData {
    pub(crate) balance: U256,
    pub(crate) nonce: U256,
    pub(crate) code: Arc<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EspaceRpcBlock {
    pub(crate) hash: H256,
    pub(crate) number: U256,
    pub(crate) base_fee_per_gas: Option<U256>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceRpcAccount {
    pub(crate) balance: U256,
    pub(crate) nonce: U256,
    pub(crate) code_hash: H256,
    pub(crate) staking_balance: U256,
    pub(crate) collateral_for_storage: U256,
    pub(crate) accumulated_interest_return: U256,
    pub(crate) admin: RpcAddress,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceSponsorInfo {
    pub(crate) sponsor_for_gas: RpcAddress,
    pub(crate) sponsor_for_collateral: RpcAddress,
    pub(crate) sponsor_gas_bound: U256,
    pub(crate) sponsor_balance_for_gas: U256,
    pub(crate) sponsor_balance_for_collateral: U256,
    pub(crate) available_storage_points: U256,
    pub(crate) used_storage_points: U256,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CoreSpaceRpcBlock {
    pub(crate) hash: H256,
    pub(crate) height: U256,
    pub(crate) miner: RpcAddress,
    pub(crate) block_number: Option<U256>,
    pub(crate) base_fee_per_gas: Option<U256>,
    pub(crate) timestamp: U256,
}

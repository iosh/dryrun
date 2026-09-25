use std::sync::Arc;

use alloy_primitives::B256;
use cfx_types::{Address, H256, U64, U256};

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceSupplyInfo {
    pub(crate) total_issued: U256,
    pub(crate) total_staking: U256,
    pub(crate) total_espace_tokens: U256,
    pub(crate) total_collateral: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceStorageCollateralInfo {
    pub(crate) converted_storage_points: U256,
    pub(crate) used_storage_points: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpacePoSEconomics {
    pub(crate) total_pos_staking_tokens: U256,
    pub(crate) distributable_pos_interest: U256,
    pub(crate) last_distribute_block: U64,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub(crate) struct EspaceRpcBlock {
    pub(crate) hash: B256,
    pub(crate) number: u64,
    pub(crate) base_fee_per_gas: Option<U256>,
}
#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceRpcAccount {
    pub(crate) balance: U256,
    pub(crate) nonce: U256,
    pub(crate) code_hash: H256,
    pub(crate) staking_balance: U256,
    pub(crate) total_collateral_for_storage: U256,
    pub(crate) accumulated_interest_return: U256,
    pub(crate) admin: Address,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceAccountState {
    pub(crate) account: CoreSpaceRpcAccount,
    pub(crate) token_collateral_for_storage: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceSponsorInfo {
    pub(crate) sponsor_for_gas: Address,
    pub(crate) sponsor_for_collateral: Address,
    pub(crate) sponsor_gas_bound: U256,
    pub(crate) sponsor_balance_for_gas: U256,
    pub(crate) sponsor_balance_for_collateral: U256,
    pub(crate) available_storage_point_units: U256,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceRpcBlock {
    pub(crate) hash: H256,
    pub(crate) epoch_number: Option<U256>,
    pub(crate) miner: Address,
    pub(crate) block_number: Option<U256>,
    pub(crate) base_fee_per_gas: Option<U256>,
    pub(crate) timestamp: U256,
    pub(crate) pos_reference: Option<H256>,
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpaceRpcPoSBlock {
    pub(crate) height: U64,
    pub(crate) pivot_decision: Option<CoreSpaceRpcPoSPivotDecision>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CoreSpaceRpcPoSPivotDecision {
    pub(crate) height: U64,
}

use std::collections::BTreeMap;

use cfx_executor::spec::CommonParams;
use cfx_internal_common::ChainIdParamsInner;
use cfx_parameters::{
    block::{EVM_TRANSACTION_BLOCK_RATIO, EVM_TRANSACTION_GAS_RATIO},
    consensus::{BN128_ENABLE_NUMBER, TANZANITE_HEIGHT},
    consensus_internal::{
        ANTICONE_PENALTY_RATIO, DAO_PARAMETER_VOTE_PERIOD, INITIAL_1559_CORE_BASE_PRICE,
        INITIAL_1559_ETH_BASE_PRICE, INITIAL_BASE_MINING_REWARD_IN_UCFX,
        MINING_REWARD_TANZANITE_IN_UCFX,
    },
};
use cfx_types::{AllChainID, SpaceMap, U256};
use primitives::block_header::CIP112_TRANSITION_HEIGHT;

/// Pinned mainnet params must stay aligned with the current upstream commit.
///
/// Upstream keeps some config state outside `CommonParams` itself, so we mirror
/// that here instead of relying on `cfx_config`.
pub fn pinned_mainnet_common_params() -> CommonParams {
    const MAINNET_NATIVE_CHAIN_ID: u32 = 1029;
    const MAINNET_EVM_CHAIN_ID: u32 = 1030;
    const MAINNET_NETWORK_ID: u64 = 1029;

    const HYDRA_TRANSITION_NUMBER: u64 = 92_060_600;
    const HYDRA_TRANSITION_HEIGHT: u64 = 36_935_000;
    const CIP43_INIT_END_NUMBER: u64 = 92_751_800;

    let cip112_transition_height = *CIP112_TRANSITION_HEIGHT.get_or_init(|| u64::MAX);

    let mut params = CommonParams::default();

    params.network_id = MAINNET_NETWORK_ID;
    params.chain_id = ChainIdParamsInner::new_simple(AllChainID::new(
        MAINNET_NATIVE_CHAIN_ID,
        MAINNET_EVM_CHAIN_ID,
    ));
    params.min_base_price =
        SpaceMap::new(INITIAL_1559_CORE_BASE_PRICE, INITIAL_1559_ETH_BASE_PRICE)
            .map_all(U256::from);

    params.anticone_penalty_ratio = ANTICONE_PENALTY_RATIO;
    params.evm_transaction_block_ratio = EVM_TRANSACTION_BLOCK_RATIO;
    params.evm_transaction_gas_ratio = EVM_TRANSACTION_GAS_RATIO;
    params.params_dao_vote_period = DAO_PARAMETER_VOTE_PERIOD;
    params.base_block_rewards = BTreeMap::from([
        (0, INITIAL_BASE_MINING_REWARD_IN_UCFX.into()),
        (TANZANITE_HEIGHT, MINING_REWARD_TANZANITE_IN_UCFX.into()),
    ]);

    params.transition_heights.cip40 = TANZANITE_HEIGHT;

    params.transition_numbers.cip43a = HYDRA_TRANSITION_NUMBER;
    params.transition_numbers.cip64 = HYDRA_TRANSITION_NUMBER;
    params.transition_numbers.cip71 = HYDRA_TRANSITION_NUMBER;
    params.transition_numbers.cip78a = HYDRA_TRANSITION_NUMBER;
    params.transition_numbers.cip92 = HYDRA_TRANSITION_NUMBER;
    params.transition_heights.cip76 = HYDRA_TRANSITION_HEIGHT;
    params.transition_heights.cip86 = HYDRA_TRANSITION_HEIGHT;
    params.transition_numbers.cip43b = CIP43_INIT_END_NUMBER;
    params.transition_numbers.cip62 = BN128_ENABLE_NUMBER;
    params.transition_numbers.cip78b = params.transition_numbers.cip78a;
    params.transition_heights.cip90a = HYDRA_TRANSITION_HEIGHT;
    params.transition_numbers.cip90b = HYDRA_TRANSITION_NUMBER;

    params.transition_numbers.cip94n = u64::MAX;
    params.transition_heights.cip94h = u64::MAX;
    params.transition_numbers.cip97 = u64::MAX;
    params.transition_numbers.cip98 = u64::MAX;
    params.transition_numbers.cip105 = u64::MAX;
    params.transition_numbers.cip_sigma_fix = u64::MAX;
    params.transition_numbers.cip107 = u64::MAX;
    params.transition_heights.cip112 = cip112_transition_height;
    params.transition_numbers.cip118 = u64::MAX;
    params.transition_numbers.cip119 = u64::MAX;

    params.transition_numbers.cip131 = u64::MAX;
    params.transition_numbers.cip132 = u64::MAX;
    params.transition_numbers.cip133b = u64::MAX;
    params.transition_numbers.cip137 = u64::MAX;
    params.transition_numbers.cancun_opcodes = u64::MAX;
    params.transition_numbers.cip144 = u64::MAX;
    params.transition_numbers.cip145 = u64::MAX;

    params.transition_heights.cip130 = u64::MAX;
    params.transition_heights.cip133e = u64::MAX;
    params.transition_heights.cip1559 = u64::MAX;
    params.transition_heights.cip150 = u64::MAX;
    params.transition_heights.cip151 = u64::MAX;
    params.transition_heights.cip152 = u64::MAX;
    params.transition_heights.cip154 = u64::MAX;
    params.transition_heights.cip7702 = u64::MAX;
    params.transition_heights.cip645 = u64::MAX;
    params.transition_heights.align_evm = u64::MAX;
    params.transition_heights.eip2935 = u64::MAX;
    params.transition_heights.eip2537 = u64::MAX;
    params.transition_heights.eip7623 = u64::MAX;
    params.transition_heights.cip_c2_fix = u64::MAX;
    params.transition_heights.cip145_fix = u64::MAX;
    params.transition_heights.cip166 = u64::MAX;

    params
}

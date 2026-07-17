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

/// Mainnet params mirrored from the current upstream commit.
///
/// Upstream keeps some config state outside `CommonParams` itself, so we mirror
/// that here instead of relying on `cfx_config`.
pub fn mainnet_common_params() -> CommonParams {
    const MAINNET_NATIVE_CHAIN_ID: u32 = 1029;
    const MAINNET_EVM_CHAIN_ID: u32 = 1030;
    const MAINNET_NETWORK_ID: u64 = 1029;

    const HYDRA_TRANSITION_NUMBER: u64 = 92_060_600;
    const HYDRA_TRANSITION_HEIGHT: u64 = 36_935_000;
    const CIP43_INIT_END_NUMBER: u64 = 92_751_800;
    const DAO_VOTE_TRANSITION_NUMBER: u64 = 133_800_000;
    const DAO_VOTE_TRANSITION_HEIGHT: u64 = 56_800_000;
    const SIGMA_FIX_TRANSITION_NUMBER: u64 = 137_740_000;
    const BURN_COLLATERAL_TRANSITION_NUMBER: u64 = 188_900_000;
    const CIP112_TRANSITION_HEIGHT_DEFAULT: u64 = 79_050_000;
    const BASE_FEE_BURN_TRANSITION_NUMBER: u64 = 247_480_000;
    const BASE_FEE_BURN_TRANSITION_HEIGHT: u64 = 101_900_000;
    const C2_FIX_TRANSITION_HEIGHT: u64 = 118_580_000;
    const EOA_CODE_TRANSITION_HEIGHT: u64 = 129_680_000;
    let cip112_transition_height =
        *CIP112_TRANSITION_HEIGHT.get_or_init(|| CIP112_TRANSITION_HEIGHT_DEFAULT);

    let mut params = CommonParams {
        network_id: MAINNET_NETWORK_ID,
        chain_id: ChainIdParamsInner::new_simple(AllChainID::new(
            MAINNET_NATIVE_CHAIN_ID,
            MAINNET_EVM_CHAIN_ID,
        )),
        min_base_price: SpaceMap::new(INITIAL_1559_CORE_BASE_PRICE, INITIAL_1559_ETH_BASE_PRICE)
            .map_all(U256::from),
        anticone_penalty_ratio: ANTICONE_PENALTY_RATIO,
        evm_transaction_block_ratio: EVM_TRANSACTION_BLOCK_RATIO,
        evm_transaction_gas_ratio: EVM_TRANSACTION_GAS_RATIO,
        params_dao_vote_period: DAO_PARAMETER_VOTE_PERIOD,
        base_block_rewards: BTreeMap::from([
            (0, INITIAL_BASE_MINING_REWARD_IN_UCFX.into()),
            (TANZANITE_HEIGHT, MINING_REWARD_TANZANITE_IN_UCFX.into()),
        ]),
        ..Default::default()
    };

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

    params.transition_numbers.cip94n = DAO_VOTE_TRANSITION_NUMBER;
    params.transition_heights.cip94h = DAO_VOTE_TRANSITION_HEIGHT;
    params.transition_numbers.cip97 = DAO_VOTE_TRANSITION_NUMBER;
    params.transition_numbers.cip98 = DAO_VOTE_TRANSITION_NUMBER;
    params.transition_numbers.cip105 = DAO_VOTE_TRANSITION_NUMBER;
    params.transition_numbers.cip_sigma_fix = SIGMA_FIX_TRANSITION_NUMBER;
    params.transition_numbers.cip107 = BURN_COLLATERAL_TRANSITION_NUMBER;
    params.transition_heights.cip112 = cip112_transition_height;
    params.transition_numbers.cip118 = BURN_COLLATERAL_TRANSITION_NUMBER;
    params.transition_numbers.cip119 = BURN_COLLATERAL_TRANSITION_NUMBER;

    params.transition_numbers.cip131 = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cip132 = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cip133b = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cip137 = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cancun_opcodes = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cip144 = BASE_FEE_BURN_TRANSITION_NUMBER;
    params.transition_numbers.cip145 = BASE_FEE_BURN_TRANSITION_NUMBER;

    params.transition_heights.cip130 = BASE_FEE_BURN_TRANSITION_HEIGHT;
    params.transition_heights.cip133e = BASE_FEE_BURN_TRANSITION_HEIGHT;
    params.transition_heights.cip1559 = BASE_FEE_BURN_TRANSITION_HEIGHT;
    params.transition_heights.cip150 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip151 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip152 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip154 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip7702 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip645 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.align_evm = u64::MAX;
    params.transition_heights.eip2935 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.eip2537 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.eip7623 = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip_c2_fix = C2_FIX_TRANSITION_HEIGHT;
    params.transition_heights.cip145_fix = EOA_CODE_TRANSITION_HEIGHT;
    params.transition_heights.cip166 = u64::MAX;

    params
}

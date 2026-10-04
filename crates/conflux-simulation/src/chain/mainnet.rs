use std::collections::BTreeMap;

use cfx_executor::spec::{CommonParams, TransitionsBlockNumber, TransitionsEpochHeight};
use cfx_internal_common::ChainIdParamsInner;
use cfx_parameters::{
    consensus::{BN128_ENABLE_NUMBER, TANZANITE_HEIGHT},
    consensus_internal::{
        INITIAL_1559_CORE_BASE_PRICE, INITIAL_1559_ETH_BASE_PRICE,
        INITIAL_BASE_MINING_REWARD_IN_UCFX, MINING_REWARD_TANZANITE_IN_UCFX,
    },
};
use cfx_types::{AllChainID, SpaceMap, U256};

const MAINNET_CORE_SPACE_CHAIN_ID: u32 = 1029;
const MAINNET_ESPACE_CHAIN_ID: u32 = 1030;
const MAINNET_NETWORK_ID: u64 = 1029;

/// Conflux mainnet execution parameters for the upstream revision locked by `Cargo.lock`.
// Source: conflux-rust crates/config/src/configuration.rs (hardfork defaults and
// set_cips), with chain IDs from run/hydra.toml. Keep the transition tables
// exhaustive so new upstream gates require review instead of defaulting to zero.
pub(super) fn params() -> CommonParams {
    const HYDRA_TRANSITION_NUMBER: u64 = 92_060_600;
    const HYDRA_TRANSITION_HEIGHT: u64 = 36_935_000;
    const CIP43_INIT_END_NUMBER: u64 = 92_751_800;
    const DAO_VOTE_TRANSITION_NUMBER: u64 = 133_800_000;
    const DAO_VOTE_TRANSITION_HEIGHT: u64 = 56_800_000;
    const SIGMA_FIX_TRANSITION_NUMBER: u64 = 137_740_000;
    const BURN_COLLATERAL_TRANSITION_NUMBER: u64 = 188_900_000;
    const CIP112_TRANSITION_HEIGHT: u64 = 79_050_000;
    const BASE_FEE_BURN_TRANSITION_NUMBER: u64 = 247_480_000;
    const BASE_FEE_BURN_TRANSITION_HEIGHT: u64 = 101_900_000;
    const C2_FIX_TRANSITION_HEIGHT: u64 = 118_580_000;
    const EOA_CODE_TRANSITION_HEIGHT: u64 = 129_680_000;
    const OSAKA_OPCODE_TRANSITION_HEIGHT: u64 = 155_140_000;

    CommonParams {
        network_id: MAINNET_NETWORK_ID,
        chain_id: ChainIdParamsInner::new_simple(AllChainID::new(
            MAINNET_CORE_SPACE_CHAIN_ID,
            MAINNET_ESPACE_CHAIN_ID,
        )),
        min_base_price: SpaceMap::new(INITIAL_1559_CORE_BASE_PRICE, INITIAL_1559_ETH_BASE_PRICE)
            .map_all(U256::from),
        base_block_rewards: BTreeMap::from([
            (0, INITIAL_BASE_MINING_REWARD_IN_UCFX.into()),
            (TANZANITE_HEIGHT, MINING_REWARD_TANZANITE_IN_UCFX.into()),
        ]),
        transition_numbers: TransitionsBlockNumber {
            cip43a: HYDRA_TRANSITION_NUMBER,
            cip43b: CIP43_INIT_END_NUMBER,
            cip62: BN128_ENABLE_NUMBER,
            cip64: HYDRA_TRANSITION_NUMBER,
            cip71: HYDRA_TRANSITION_NUMBER,
            cip78a: HYDRA_TRANSITION_NUMBER,
            cip78b: HYDRA_TRANSITION_NUMBER,
            cip90b: HYDRA_TRANSITION_NUMBER,
            cip92: HYDRA_TRANSITION_NUMBER,
            cip94n: DAO_VOTE_TRANSITION_NUMBER,
            cip97: DAO_VOTE_TRANSITION_NUMBER,
            cip98: DAO_VOTE_TRANSITION_NUMBER,
            cip105: DAO_VOTE_TRANSITION_NUMBER,
            cip107: BURN_COLLATERAL_TRANSITION_NUMBER,
            cip_sigma_fix: SIGMA_FIX_TRANSITION_NUMBER,
            cip118: BURN_COLLATERAL_TRANSITION_NUMBER,
            cip119: BURN_COLLATERAL_TRANSITION_NUMBER,
            cip131: BASE_FEE_BURN_TRANSITION_NUMBER,
            cip132: BASE_FEE_BURN_TRANSITION_NUMBER,
            cip133b: BASE_FEE_BURN_TRANSITION_NUMBER,
            cip137: BASE_FEE_BURN_TRANSITION_NUMBER,
            cancun_opcodes: BASE_FEE_BURN_TRANSITION_NUMBER,
            cip144: BASE_FEE_BURN_TRANSITION_NUMBER,
            cip145: BASE_FEE_BURN_TRANSITION_NUMBER,
        },
        transition_heights: TransitionsEpochHeight {
            cip40: TANZANITE_HEIGHT,
            cip76: HYDRA_TRANSITION_HEIGHT,
            cip86: HYDRA_TRANSITION_HEIGHT,
            cip90a: HYDRA_TRANSITION_HEIGHT,
            cip94h: DAO_VOTE_TRANSITION_HEIGHT,
            cip112: CIP112_TRANSITION_HEIGHT,
            cip130: BASE_FEE_BURN_TRANSITION_HEIGHT,
            cip133e: BASE_FEE_BURN_TRANSITION_HEIGHT,
            cip1559: BASE_FEE_BURN_TRANSITION_HEIGHT,
            cip150: EOA_CODE_TRANSITION_HEIGHT,
            cip151: EOA_CODE_TRANSITION_HEIGHT,
            cip152: EOA_CODE_TRANSITION_HEIGHT,
            cip154: EOA_CODE_TRANSITION_HEIGHT,
            cip7702: EOA_CODE_TRANSITION_HEIGHT,
            cip645: EOA_CODE_TRANSITION_HEIGHT,
            align_evm: u64::MAX,
            eip2935: EOA_CODE_TRANSITION_HEIGHT,
            eip2537: EOA_CODE_TRANSITION_HEIGHT,
            eip7623: EOA_CODE_TRANSITION_HEIGHT,
            cip_c2_fix: C2_FIX_TRANSITION_HEIGHT,
            cip145_fix: EOA_CODE_TRANSITION_HEIGHT,
            cip166: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip167: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip172: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip174: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip175: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip176: OSAKA_OPCODE_TRANSITION_HEIGHT,
            cip_hn_fix: OSAKA_OPCODE_TRANSITION_HEIGHT,
        },
        ..Default::default()
    }
}

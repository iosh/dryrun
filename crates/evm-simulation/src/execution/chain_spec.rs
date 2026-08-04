use alloy_chains::Chain;
use alloy_hardforks::EthereumHardfork;
use revm::primitives::hardfork::SpecId;

use super::EvmExecutionError;

pub(super) fn resolve_execution_spec_id(
    chain_id: u64,
    block_number: u64,
    timestamp: u64,
) -> Result<SpecId, EvmExecutionError> {
    if chain_id != Chain::mainnet().id() {
        return Err(EvmExecutionError::UnsupportedChain(chain_id));
    }

    let hardfork = EthereumHardfork::mainnet()
        .iter()
        .rev()
        .find_map(|(hardfork, condition)| {
            condition
                .active_at_timestamp_or_number(timestamp, block_number)
                .then_some(*hardfork)
        })
        .unwrap_or(EthereumHardfork::Frontier);

    map_hardfork_to_spec_id(hardfork)
}

fn map_hardfork_to_spec_id(hardfork: EthereumHardfork) -> Result<SpecId, EvmExecutionError> {
    let spec_id = match hardfork {
        EthereumHardfork::Frontier => SpecId::FRONTIER,
        EthereumHardfork::Homestead => SpecId::HOMESTEAD,
        EthereumHardfork::Dao => SpecId::DAO_FORK,
        EthereumHardfork::Tangerine => SpecId::TANGERINE,
        EthereumHardfork::SpuriousDragon => SpecId::SPURIOUS_DRAGON,
        EthereumHardfork::Byzantium => SpecId::BYZANTIUM,
        EthereumHardfork::Constantinople => SpecId::CONSTANTINOPLE,
        EthereumHardfork::Petersburg => SpecId::PETERSBURG,
        EthereumHardfork::Istanbul => SpecId::ISTANBUL,
        EthereumHardfork::MuirGlacier => SpecId::MUIR_GLACIER,
        EthereumHardfork::Berlin => SpecId::BERLIN,
        EthereumHardfork::London => SpecId::LONDON,
        EthereumHardfork::ArrowGlacier => SpecId::ARROW_GLACIER,
        EthereumHardfork::GrayGlacier => SpecId::GRAY_GLACIER,
        EthereumHardfork::Paris => SpecId::MERGE,
        EthereumHardfork::Shanghai => SpecId::SHANGHAI,
        EthereumHardfork::Cancun => SpecId::CANCUN,
        EthereumHardfork::Prague => SpecId::PRAGUE,
        EthereumHardfork::Osaka
        | EthereumHardfork::Bpo1
        | EthereumHardfork::Bpo2
        | EthereumHardfork::Bpo3
        | EthereumHardfork::Bpo4
        | EthereumHardfork::Bpo5 => SpecId::OSAKA,
        EthereumHardfork::Amsterdam => SpecId::AMSTERDAM,
        _ => {
            return Err(EvmExecutionError::UnsupportedHardfork(format!(
                "{hardfork:?}"
            )));
        }
    };

    Ok(spec_id)
}

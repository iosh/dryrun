use alloy_chains::Chain;
use alloy_hardforks::EthereumHardfork;
use revm::primitives::hardfork::SpecId;

use crate::EvmEngineError;

pub(crate) fn resolve_execution_spec_id(
    chain_id: u64,
    block_number: u64,
    timestamp: u64,
) -> Result<SpecId, EvmEngineError> {
    validate_supported_chain_id(chain_id)?;

    let hardfork = resolve_mainnet_hardfork(block_number, timestamp);
    map_hardfork_to_spec_id(hardfork)
}

fn validate_supported_chain_id(chain_id: u64) -> Result<(), EvmEngineError> {
    if chain_id == Chain::mainnet().id() {
        return Ok(());
    }

    Err(EvmEngineError::not_supported(format!(
        "only Ethereum mainnet is supported now, got chain_id={chain_id}"
    )))
}

fn resolve_mainnet_hardfork(block_number: u64, timestamp: u64) -> EthereumHardfork {
    for hardfork in [
        EthereumHardfork::Amsterdam,
        EthereumHardfork::Bpo5,
        EthereumHardfork::Bpo4,
        EthereumHardfork::Bpo3,
        EthereumHardfork::Bpo2,
        EthereumHardfork::Bpo1,
        EthereumHardfork::Osaka,
        EthereumHardfork::Prague,
        EthereumHardfork::Cancun,
        EthereumHardfork::Shanghai,
    ] {
        if is_mainnet_timestamp_fork_active(hardfork, timestamp) {
            return hardfork;
        }
    }

    EthereumHardfork::from_mainnet_block_number(block_number)
}

fn is_mainnet_timestamp_fork_active(hardfork: EthereumHardfork, timestamp: u64) -> bool {
    hardfork
        .activation_timestamp(Chain::mainnet())
        .is_some_and(|activation_timestamp| timestamp >= activation_timestamp)
}

fn map_hardfork_to_spec_id(hardfork: EthereumHardfork) -> Result<SpecId, EvmEngineError> {
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
            return Err(EvmEngineError::not_ready(format!(
                "hardfork {hardfork:?} is not mapped to revm::SpecId yet"
            )));
        }
    };

    Ok(spec_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_spec_id_respects_block_based_activation() {
        let chain = Chain::mainnet();
        let london_block = EthereumHardfork::London
            .activation_block(chain)
            .expect("mainnet london block should exist");
        let london_timestamp = EthereumHardfork::London
            .activation_timestamp(chain)
            .expect("mainnet london timestamp should exist");

        let spec_id =
            resolve_execution_spec_id(chain.id(), london_block.saturating_sub(1), london_timestamp)
                .expect("spec id should resolve");

        assert_eq!(spec_id, SpecId::BERLIN);
    }

    #[test]
    fn resolve_spec_id_respects_timestamp_based_activation() {
        let chain = Chain::mainnet();
        let shanghai_timestamp = EthereumHardfork::Shanghai
            .activation_timestamp(chain)
            .expect("mainnet shanghai timestamp should exist");

        let spec_id = resolve_execution_spec_id(
            chain.id(),
            EthereumHardfork::Paris
                .activation_block(chain)
                .expect("mainnet paris block should exist"),
            shanghai_timestamp,
        )
        .expect("spec id should resolve");

        assert_eq!(spec_id, SpecId::SHANGHAI);
    }

    #[test]
    fn resolve_spec_id_rejects_non_mainnet_chain() {
        let error = resolve_execution_spec_id(11155111, 1, 1).unwrap_err();

        assert!(matches!(
            error,
            EvmEngineError::NotSupported(message)
                if message.contains("only Ethereum mainnet is supported now")
        ));
    }
}

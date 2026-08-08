use alloy_chains::Chain;
use alloy_hardforks::EthereumHardfork;
use revm::primitives::hardfork::SpecId;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EthereumChainSpec {
    chain_id: u64,
}

impl EthereumChainSpec {
    pub(crate) const fn mainnet() -> Self {
        Self {
            chain_id: Chain::mainnet().id(),
        }
    }

    pub(crate) const fn chain_id(self) -> u64 {
        self.chain_id
    }

    pub(crate) const fn native_currency(self) -> NativeCurrency {
        NativeCurrency {
            name: "Ether",
            symbol: "ETH",
            decimals: 18,
        }
    }

    pub(crate) fn execution_spec_id(
        self,
        block_number: u64,
        timestamp: u64,
    ) -> Result<SpecId, EthereumChainSpecError> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativeCurrency {
    pub(crate) name: &'static str,
    pub(crate) symbol: &'static str,
    pub(crate) decimals: u8,
}

#[derive(Debug, Error)]
pub(crate) enum EthereumChainSpecError {
    #[error("hardfork {hardfork:?} is not mapped to revm::SpecId yet")]
    UnsupportedHardfork { hardfork: EthereumHardfork },
}

fn map_hardfork_to_spec_id(hardfork: EthereumHardfork) -> Result<SpecId, EthereumChainSpecError> {
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
        _ => return Err(EthereumChainSpecError::UnsupportedHardfork { hardfork }),
    };

    Ok(spec_id)
}

use alloy::eips::eip7840::BlobParams;
use alloy_chains::Chain;
use alloy_hardforks::{EthereumChainHardforks, EthereumHardfork, EthereumHardforks};
use revm::primitives::hardfork::SpecId;
use thiserror::Error;

use crate::changeset::EvmNativeCurrency;

#[derive(Debug, Clone)]
pub(crate) struct EthereumChainSpec {
    chain: Chain,
    hardforks: EthereumChainHardforks,
    native_currency: EvmNativeCurrency,
    wrapped_native_token: Option<alloy::primitives::Address>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EthereumExecutionSpec {
    pub(crate) spec_id: SpecId,
    pub(crate) blob_params: Option<BlobParams>,
}

impl EthereumChainSpec {
    pub(crate) fn mainnet() -> Self {
        let chain = Chain::mainnet();
        Self {
            chain,
            hardforks: EthereumChainHardforks::mainnet(),
            native_currency: EvmNativeCurrency {
                name: "Ether".to_string(),
                symbol: "ETH".to_string(),
                decimals: 18,
            },
            wrapped_native_token: chain
                .named()
                .and_then(|network| network.wrapped_native_token()),
        }
    }

    pub(crate) const fn chain_id(&self) -> u64 {
        self.chain.id()
    }

    pub(crate) const fn native_currency(&self) -> &EvmNativeCurrency {
        &self.native_currency
    }

    pub(crate) const fn wrapped_native_token_address(&self) -> Option<alloy::primitives::Address> {
        self.wrapped_native_token
    }

    pub(crate) fn execution_spec(
        &self,
        block_number: u64,
        timestamp: u64,
    ) -> Result<EthereumExecutionSpec, EthereumChainSpecError> {
        let hardfork = EthereumHardfork::VARIANTS
            .iter()
            .rev()
            .find(|hardfork| self.is_active(**hardfork, block_number, timestamp))
            .copied()
            .unwrap_or(EthereumHardfork::Frontier);

        let spec_id = map_hardfork_to_spec_id(hardfork)?;
        let blob_params = self.blob_params(block_number, timestamp)?;

        Ok(EthereumExecutionSpec {
            spec_id,
            blob_params,
        })
    }

    fn blob_params(
        &self,
        block_number: u64,
        timestamp: u64,
    ) -> Result<Option<BlobParams>, EthereumChainSpecError> {
        for hardfork in [
            EthereumHardfork::Bpo5,
            EthereumHardfork::Bpo4,
            EthereumHardfork::Bpo3,
        ] {
            if self.is_active(hardfork, block_number, timestamp) {
                return Err(EthereumChainSpecError::UnsupportedHardfork { hardfork });
            }
        }

        let params = if self.is_active(EthereumHardfork::Bpo2, block_number, timestamp) {
            Some(BlobParams::bpo2())
        } else if self.is_active(EthereumHardfork::Bpo1, block_number, timestamp) {
            Some(BlobParams::bpo1())
        } else if self.is_active(EthereumHardfork::Osaka, block_number, timestamp) {
            Some(BlobParams::osaka())
        } else if self.is_active(EthereumHardfork::Prague, block_number, timestamp) {
            Some(BlobParams::prague())
        } else if self.is_active(EthereumHardfork::Cancun, block_number, timestamp) {
            Some(BlobParams::cancun())
        } else {
            None
        };

        Ok(params)
    }

    fn is_active(&self, hardfork: EthereumHardfork, block_number: u64, timestamp: u64) -> bool {
        self.hardforks
            .ethereum_fork_activation(hardfork)
            .active_at_timestamp_or_number(timestamp, block_number)
    }
}

#[derive(Debug, Error)]
pub(crate) enum EthereumChainSpecError {
    #[error("hardfork {hardfork:?} is not fully supported by the EVM executor")]
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

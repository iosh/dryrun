use alloy::consensus::BlockHeader;
use alloy::eips::{eip7840::BlobParams, eip7892::BlobScheduleBlobParams};
use alloy_evm::EvmEnv;
use alloy_hardforks::{
    EthereumChainHardforks, EthereumHardforks,
    mainnet::{MAINNET_BPO1_TIMESTAMP, MAINNET_BPO2_TIMESTAMP},
};

/// Everything that differs between EVM networks with Ethereum's execution rules.
#[derive(Debug, Clone)]
pub struct ChainSpec {
    pub chain_id: u64,
    pub hardforks: EthereumChainHardforks,
    pub blob_schedule: BlobScheduleBlobParams,
}

impl ChainSpec {
    pub fn mainnet() -> Self {
        Self {
            chain_id: 1,
            hardforks: EthereumChainHardforks::mainnet(),
            blob_schedule: BlobScheduleBlobParams::mainnet().with_scheduled([
                (MAINNET_BPO1_TIMESTAMP, BlobParams::bpo1()),
                (MAINNET_BPO2_TIMESTAMP, BlobParams::bpo2()),
            ]),
        }
    }

    /// The VM environment of a block, equal to the one it was executed in.
    pub(crate) fn evm_env(&self, header: impl BlockHeader) -> EvmEnv {
        let blob_params = self.blob_params(header.timestamp());
        EvmEnv::for_eth_block(header, &self.hardforks, self.chain_id, blob_params)
    }

    fn blob_params(&self, timestamp: u64) -> Option<BlobParams> {
        let schedule = &self.blob_schedule;
        if self.hardforks.is_osaka_active_at_timestamp(timestamp) {
            let scheduled = schedule.active_scheduled_params_at_timestamp(timestamp);
            Some(*scheduled.unwrap_or(&schedule.osaka))
        } else if self.hardforks.is_prague_active_at_timestamp(timestamp) {
            Some(schedule.prague)
        } else if self.hardforks.is_cancun_active_at_timestamp(timestamp) {
            Some(schedule.cancun)
        } else {
            None
        }
    }
}

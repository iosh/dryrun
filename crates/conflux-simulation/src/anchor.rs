use crate::Error;
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::B256,
    providers::{DynProvider, Provider},
    rpc::types::Header,
};
use conflux_provider::{ConfluxProvider, CoreRpcBlock, EpochNumber};

#[derive(Debug, Clone, Copy)]
pub(crate) struct Anchor {
    pub epoch: u64,
    pub pivot_hash: B256,
}

impl Anchor {
    pub fn block(self) -> BlockId {
        BlockId::hash_canonical(self.pivot_hash)
    }
    pub fn epoch(self) -> EpochNumber {
        EpochNumber::Number(self.epoch)
    }

    pub async fn fetch(
        core: &ConfluxProvider,
        espace: &DynProvider,
        block: BlockId,
    ) -> Result<(Self, CoreRpcBlock, Header), Error> {
        if matches!(
            block,
            BlockId::Number(BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest)
        ) {
            return Err(Error::Unsupported(format!(
                "block {block} is not supported"
            )));
        }
        let header = espace
            .get_block(block)
            .await
            .map_err(|source| Error::EspaceProvider {
                operation: "eth_getBlock",
                source,
            })?
            .ok_or(Error::BlockNotFound(block))?
            .header;
        let anchor = Self {
            epoch: header.number,
            pivot_hash: header.hash,
        };
        let pivot = core
            .cfx_get_block_by_epoch_number(anchor.epoch(), false)
            .await?
            .ok_or(Error::BlockNotFound(block))?;
        if pivot.hash != anchor.pivot_hash {
            return Err(Error::ContextInconsistent);
        }
        Ok((anchor, pivot, header))
    }

    pub async fn check(self, core: &ConfluxProvider) -> Result<(), Error> {
        let pivot = core
            .cfx_get_block_by_epoch_number(self.epoch(), false)
            .await?;
        if pivot.is_none_or(|block| block.hash != self.pivot_hash) {
            return Err(Error::ContextInconsistent);
        }
        Ok(())
    }
}

use alloy::{
    consensus::{Header, Sealed},
    eips::BlockNumberOrTag,
    network::Ethereum,
    providers::{DynProvider, Provider},
};

use crate::{EvmBlockSelector, EvmContextError};

pub(crate) async fn resolve_block(
    provider: &DynProvider<Ethereum>,
    selector: EvmBlockSelector,
) -> Result<Sealed<Header>, EvmContextError> {
    let block = match selector {
        EvmBlockSelector::Hash(hash) => provider.get_block_by_hash(hash).await,
        EvmBlockSelector::Latest => provider.get_block_by_number(BlockNumberOrTag::Latest).await,
        EvmBlockSelector::Safe => provider.get_block_by_number(BlockNumberOrTag::Safe).await,
        EvmBlockSelector::Finalized => {
            provider
                .get_block_by_number(BlockNumberOrTag::Finalized)
                .await
        }
        EvmBlockSelector::Number(number) => {
            provider
                .get_block_by_number(BlockNumberOrTag::Number(number))
                .await
        }
    }
    .map_err(|source| EvmContextError::provider(selector, source))?
    .ok_or(EvmContextError::BlockNotFound { selector })?;

    let block_hash = block.hash();
    let block = Sealed::new_unchecked(block.into_consensus_header(), block_hash);

    Ok(block)
}

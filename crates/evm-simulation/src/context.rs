use alloy::{
    consensus::{Header, Sealed},
    eips::BlockNumberOrTag,
    network::Ethereum,
    providers::{DynProvider, Provider},
};

use crate::{EvmBlockResolutionError, EvmBlockSelector};

pub(crate) async fn resolve_block(
    provider: &DynProvider<Ethereum>,
    selector: EvmBlockSelector,
) -> Result<Sealed<Header>, EvmBlockResolutionError> {
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
    .map_err(|source| EvmBlockResolutionError::request(selector, source))?
    .ok_or(EvmBlockResolutionError::BlockNotFound { selector })?;

    let block_hash = block.hash();
    let block = Sealed::new_unchecked(block.into_consensus_header(), block_hash);

    Ok(block)
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use alloy::{
        network::Ethereum,
        primitives::B256,
        providers::{DynProvider, Provider, RootProvider},
        rpc::{client::RpcClient, types::Block},
        transports::mock::Asserter,
    };

    use super::resolve_block;
    use crate::{EvmBlockResolutionError, EvmBlockSelector};

    #[test]
    fn reports_the_unresolved_selector_as_block_not_found() {
        let selector = EvmBlockSelector::Hash(B256::repeat_byte(7));
        let asserter = Asserter::new();
        asserter.push_success(&Option::<Block>::None);
        let provider = mock_provider(asserter);

        let error = block_on(resolve_block(&provider, selector))
            .expect_err("missing block should fail resolution");

        assert!(matches!(
            error,
            EvmBlockResolutionError::BlockNotFound {
                selector: actual,
            } if actual == selector
        ));
    }

    fn mock_provider(asserter: Asserter) -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(asserter)).erased()
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }
}

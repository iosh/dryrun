use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::Block,
};
use revm::database::{AlloyDB, CacheDB, WrapDatabaseAsync};

use crate::{BlockRef, EvmEngineError};

pub(super) type AlloyCacheDb = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;

#[derive(Debug, Clone)]
pub(super) struct ResolvedExecutionBlock {
    pub(super) chain_id: u64,
    pub(super) state_block_id: BlockId,
    pub(super) block: Block,
}

pub(super) fn build_provider(rpc_url: &str) -> Result<DynProvider, EvmEngineError> {
    let rpc_url = rpc_url
        .parse()
        .map_err(|error| EvmEngineError::config_error(format!("invalid rpc url: {error}")))?;

    Ok(ProviderBuilder::new().connect_http(rpc_url).erased())
}

pub(super) async fn resolve_execution_block(
    provider: &DynProvider,
    block_ref: &BlockRef,
) -> Result<ResolvedExecutionBlock, EvmEngineError> {
    let chain_id = provider.get_chain_id().await.map_err(map_rpc_error)?;
    let (block, state_block_id) = match block_ref {
        BlockRef::Latest => {
            let block = load_block(
                provider,
                BlockId::Number(BlockNumberOrTag::Latest),
                "latest block was not returned by provider",
            )
            .await?;
            let block_number = block.number();

            (block, block_number_id(block_number))
        }
        BlockRef::Number(number) => {
            let block_id = block_number_id(*number);
            let block = load_block(
                provider,
                block_id,
                format!("block number {number} was not returned by provider"),
            )
            .await?;

            (block, block_id)
        }
        BlockRef::Hash(_) => {
            return Err(EvmEngineError::not_supported(
                "block.hash is not supported yet",
            ));
        }
    };

    Ok(ResolvedExecutionBlock {
        chain_id,
        state_block_id,
        block,
    })
}

pub(super) fn create_database(
    provider: &DynProvider,
    resolved_block: &ResolvedExecutionBlock,
) -> Result<AlloyCacheDb, EvmEngineError> {
    let alloy_db = WrapDatabaseAsync::new(AlloyDB::new(
        provider.clone(),
        resolved_block.state_block_id,
    ))
    .ok_or_else(|| {
        EvmEngineError::runtime_error(
            "failed to create async database wrapper from current tokio runtime",
        )
    })?;

    Ok(CacheDB::new(alloy_db))
}

async fn load_block(
    provider: &DynProvider,
    block_id: BlockId,
    missing_message: impl Into<String>,
) -> Result<Block, EvmEngineError> {
    provider
        .get_block(block_id)
        .await
        .map_err(map_rpc_error)?
        .ok_or_else(|| EvmEngineError::block_not_found(missing_message))
}

fn block_number_id(number: u64) -> BlockId {
    BlockId::Number(BlockNumberOrTag::Number(number))
}

fn map_rpc_error(error: impl std::fmt::Display) -> EvmEngineError {
    EvmEngineError::rpc_error(format!("rpc request failed: {error}"))
}

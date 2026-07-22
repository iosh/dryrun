use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::DynProvider,
};
use revm::database::{AlloyDB, CacheDB, WrapDatabaseAsync};
use tokio::runtime::Handle;

use crate::ResolvedBlock;

pub(super) type AlloyCacheDb = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;

pub(super) fn create_database(
    provider: &DynProvider,
    runtime_handle: &Handle,
    resolved_block: &ResolvedBlock,
) -> AlloyCacheDb {
    let state_block_id = block_number_id(resolved_block.number());
    let alloy_db = AlloyDB::new(provider.clone(), state_block_id);
    let database = WrapDatabaseAsync::with_handle(alloy_db, runtime_handle.clone());

    CacheDB::new(database)
}

fn block_number_id(number: u64) -> BlockId {
    BlockId::Number(BlockNumberOrTag::Number(number))
}

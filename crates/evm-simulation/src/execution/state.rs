use alloy::{eips::BlockId, network::Ethereum, primitives::B256, providers::DynProvider};
use revm::database::{AlloyDB, CacheDB, WrapDatabaseAsync};
use tokio::runtime::Handle;

pub(crate) type MainnetEvmDatabase =
    CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider<Ethereum>>>>;

pub(crate) fn create_database(
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    block_hash: B256,
) -> MainnetEvmDatabase {
    let block_id = BlockId::Hash(block_hash.into());
    let database = AlloyDB::new(provider, block_id);
    let database = WrapDatabaseAsync::with_handle(database, runtime_handle);

    CacheDB::new(database)
}

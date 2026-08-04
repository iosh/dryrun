use alloy::{eips::BlockId, network::Ethereum, primitives::B256, providers::RootProvider};
use revm::database::{AlloyDB, CacheDB, WrapDatabaseAsync};
use tokio::runtime::Handle;

pub type MainnetEvmDatabase = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, RootProvider>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmBlockAnchor {
    number: u64,
    hash: B256,
}

impl EvmBlockAnchor {
    pub fn new(number: u64, hash: B256) -> Self {
        Self { number, hash }
    }

    pub fn number(self) -> u64 {
        self.number
    }

    pub fn hash(self) -> B256 {
        self.hash
    }
}

#[derive(Debug)]
pub struct EvmStateSource {
    database: MainnetEvmDatabase,
    anchor: EvmBlockAnchor,
}

impl EvmStateSource {
    pub fn new(provider: RootProvider, runtime_handle: Handle, anchor: EvmBlockAnchor) -> Self {
        let block_id = BlockId::Hash(anchor.hash().into());
        let alloy_db = AlloyDB::new(provider, block_id);
        let database = WrapDatabaseAsync::with_handle(alloy_db, runtime_handle);

        Self {
            database: CacheDB::new(database),
            anchor,
        }
    }

    pub(super) fn anchor(&self) -> EvmBlockAnchor {
        self.anchor
    }

    pub(crate) fn into_database(self) -> MainnetEvmDatabase {
        self.database
    }
}

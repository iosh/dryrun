use std::cell::RefCell;

use alloy::{
    eips::BlockId,
    network::Ethereum,
    primitives::{Address, B256},
    providers::DynProvider,
};
use revm::{
    Context, MainnetEvm as RevmMainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::JournalTr,
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    handler::EvmTr,
};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::EvmStateAccessError;

pub(crate) type EvmDatabase = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider<Ethereum>>>>;
pub(crate) type MainnetEvm<INSP = ()> =
    RevmMainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, EvmDatabase>, INSP>;

pub(crate) fn create_database(
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    block_hash: B256,
) -> EvmDatabase {
    let block_id = BlockId::Hash(block_hash.into());
    let database = AlloyDB::new(provider, block_id);
    let database = WrapDatabaseAsync::with_handle(database, runtime_handle);

    CacheDB::new(database)
}

#[derive(Debug)]
pub struct EvmStateView {
    inner: RefCell<EvmStateViewInner>,
}

#[derive(Debug)]
pub struct EvmStateViews {
    before: EvmStateView,
    after: EvmStateView,
}

impl EvmStateViews {
    pub(crate) fn new<BEFORE, AFTER>(before: MainnetEvm<BEFORE>, after: MainnetEvm<AFTER>) -> Self {
        Self {
            before: EvmStateView::new(before),
            after: EvmStateView::new(after),
        }
    }

    pub const fn before(&self) -> &EvmStateView {
        &self.before
    }

    pub const fn after(&self) -> &EvmStateView {
        &self.after
    }
}

impl EvmStateView {
    fn new<INSP>(evm: MainnetEvm<INSP>) -> Self {
        let evm = evm.with_inspector(());

        Self {
            inner: RefCell::new(EvmStateViewInner::new(evm)),
        }
    }

    pub fn read_account(&self, address: Address) -> Result<EvmAccountState, EvmStateReadError> {
        let mut inner = self.inner.borrow_mut();
        let account = inner
            .evm
            .ctx_mut()
            .journaled_state
            .load_account_with_code(address)
            .map_err(EvmStateAccessError::from)
            .map_err(EvmStateReadError::from)?;
        let info = &account.info;
        let delegation = info.code.as_ref().and_then(|code| code.eip7702_address());
        Ok(EvmAccountState {
            balance: info.balance,
            nonce: info.nonce,
            delegation,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmAccountState {
    balance: alloy::primitives::U256,
    nonce: u64,
    delegation: Option<Address>,
}

impl EvmAccountState {
    pub const fn balance(&self) -> alloy::primitives::U256 {
        self.balance
    }

    pub const fn nonce(&self) -> u64 {
        self.nonce
    }

    pub const fn delegation(&self) -> Option<Address> {
        self.delegation
    }
}

#[derive(Debug)]
struct EvmStateViewInner {
    evm: MainnetEvm,
}

impl EvmStateViewInner {
    fn new(evm: MainnetEvm) -> Self {
        Self { evm }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EvmStateAccessError),
}

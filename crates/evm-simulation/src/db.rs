use std::{cell::RefCell, collections::HashMap, future::IntoFuture};

use alloy::{
    eips::{
        BlockId, BlockNumHash,
        eip2935::{HISTORY_SERVE_WINDOW, HISTORY_STORAGE_ADDRESS},
    },
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider},
    transports::TransportError,
};
use revm::{
    database_interface::{DBErrorMarker, DatabaseRef},
    state::{AccountInfo, Bytecode},
};
use simulation_core::{CodedError, ErrorCode, LimitExceeded, ReadBudget};
use thiserror::Error;
use tokio::runtime::Handle;

/// State of one block, fetched from the provider on first access and cached
/// for the rest of the request.
///
/// revm reads state synchronously; reads block on the runtime, so this must
/// be used from a blocking thread.
#[derive(Debug)]
pub(crate) struct CachedAlloyDB<'a> {
    provider: DynProvider,
    block: BlockId,
    runtime: Handle,
    budget: &'a ReadBudget,
    cache: RefCell<Cache>,
}

#[derive(Debug, Default)]
struct Cache {
    accounts: HashMap<Address, Option<AccountInfo>>,
    storage: HashMap<(Address, U256), U256>,
    block_hashes: HashMap<u64, B256>,
    code: HashMap<B256, Bytecode>,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("state request {operation} failed")]
    Provider {
        operation: &'static str,
        #[source]
        source: TransportError,
    },
    #[error(transparent)]
    LimitExceeded(#[from] LimitExceeded),
    #[error("block {0} is unavailable")]
    BlockUnavailable(u64),
    #[error("code {0} was not loaded")]
    CodeNotLoaded(B256),
}

impl DBErrorMarker for StateError {}

impl CodedError for StateError {
    fn code(&self) -> ErrorCode {
        match self {
            Self::Provider { .. } => ErrorCode::ProviderRequestFailed,
            Self::LimitExceeded(_) => ErrorCode::LimitExceeded,
            Self::BlockUnavailable(_) => ErrorCode::StateUnavailable,
            Self::CodeNotLoaded(_) => ErrorCode::Internal,
        }
    }
}

impl<'a> CachedAlloyDB<'a> {
    /// State after `block`, whose parent is `parent`.
    pub(crate) fn new(
        provider: DynProvider,
        block: B256,
        parent: Option<BlockNumHash>,
        runtime: Handle,
        budget: &'a ReadBudget,
    ) -> Self {
        let mut cache = Cache::default();
        if let Some(parent) = parent {
            cache.block_hashes.insert(parent.number, parent.hash);
        }
        Self {
            provider,
            block: BlockId::hash_canonical(block),
            runtime,
            budget,
            cache: RefCell::new(cache),
        }
    }

    fn fetch<T>(
        &self,
        operation: &'static str,
        request: impl IntoFuture<Output = Result<T, TransportError>>,
    ) -> Result<T, StateError> {
        self.budget.record_state_read()?;
        self.runtime
            .block_on(request.into_future())
            .map_err(|source| StateError::Provider { operation, source })
    }

    /// The hash of an ancestor, found by following parent hashes back from
    /// the closest known descendant. The parent is always known, and the VM
    /// only asks for the last 256 blocks.
    fn ancestor_hash(&self, number: u64) -> Result<B256, StateError> {
        let known = {
            let cache = self.cache.borrow();
            (number + 1..=number + 256)
                .find_map(|child| Some((child, *cache.block_hashes.get(&child)?)))
        };
        let (mut child, mut hash) = known.ok_or(StateError::BlockUnavailable(number))?;
        while child > number {
            hash = self
                .fetch("eth_getBlockByHash", self.provider.get_block_by_hash(hash))?
                .ok_or(StateError::BlockUnavailable(child))?
                .header
                .parent_hash;
            child -= 1;
            self.cache.borrow_mut().block_hashes.insert(child, hash);
        }
        Ok(hash)
    }
}

impl DatabaseRef for CachedAlloyDB<'_> {
    type Error = StateError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account) = self.cache.borrow().accounts.get(&address) {
            return Ok(account.clone());
        }
        let provider = &self.provider;
        let (balance, nonce, code) = self.fetch(
            "eth_getBalance/eth_getTransactionCount/eth_getCode",
            async {
                tokio::try_join!(
                    provider
                        .get_balance(address)
                        .block_id(self.block)
                        .into_future(),
                    provider
                        .get_transaction_count(address)
                        .block_id(self.block)
                        .into_future(),
                    provider
                        .get_code_at(address)
                        .block_id(self.block)
                        .into_future(),
                )
            },
        )?;
        let code = Bytecode::new_raw(code);
        let code_hash = code.hash_slow();
        let account = AccountInfo::new(balance, nonce, code_hash, code.clone());
        // Since EIP-161 the VM treats an empty account as a missing one, except
        // for the EIP-7702 refund, which came after empty accounts were
        // cleared from the state. Earlier blocks are not simulated.
        let account = (!account.is_empty()).then_some(account);
        let mut cache = self.cache.borrow_mut();
        cache.code.insert(code_hash, code);
        cache.accounts.insert(address, account.clone());
        Ok(account)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Code is loaded together with its account.
        self.cache
            .borrow()
            .code
            .get(&code_hash)
            .cloned()
            .ok_or(StateError::CodeNotLoaded(code_hash))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        if let Some(value) = self.cache.borrow().storage.get(&(address, index)) {
            return Ok(*value);
        }
        let value = self.fetch(
            "eth_getStorageAt",
            self.provider
                .get_storage_at(address, index)
                .block_id(self.block),
        )?;
        self.cache
            .borrow_mut()
            .storage
            .insert((address, index), value);
        Ok(value)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        if let Some(hash) = self.cache.borrow().block_hashes.get(&number) {
            return Ok(*hash);
        }
        // Both sources follow the chain of this block even if a reorg replaces
        // it: the state keeps the hashes of recent blocks since EIP-2935, and
        // earlier ones come from the parent hashes.
        let slot = U256::from(number % HISTORY_SERVE_WINDOW as u64);
        let mut hash = B256::from(self.storage_ref(HISTORY_STORAGE_ADDRESS, slot)?);
        if hash.is_zero() {
            hash = self.ancestor_hash(number)?;
        }
        self.cache.borrow_mut().block_hashes.insert(number, hash);
        Ok(hash)
    }
}

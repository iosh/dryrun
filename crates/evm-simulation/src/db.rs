use std::{cell::RefCell, collections::HashMap, future::IntoFuture, sync::Arc};

use alloy::{
    eips::{
        BlockId,
        eip2935::{HISTORY_SERVE_WINDOW, HISTORY_STORAGE_ADDRESS},
    },
    primitives::{Address, B256, U256},
    providers::{DynProvider, Provider},
    rpc::types::Header,
    transports::TransportError,
};
use revm::{
    database_interface::{DBErrorMarker, DatabaseRef},
    state::{AccountInfo, Bytecode},
};
use simulation_core::{CodedError, ErrorCode, LimitExceeded, Limits, ReadBudget};
use thiserror::Error;
use tokio::runtime::Handle;

/// Request-local database backed by one fixed block. All reads, caching and
/// state-read accounting live here; the VM adapter only bridges async access.
#[derive(Debug)]
pub(crate) struct ForkDatabase {
    provider: DynProvider,
    block: BlockId,
    budget: Arc<ReadBudget>,
    accounts: HashMap<Address, Option<AccountInfo>>,
    storage: HashMap<(Address, U256), U256>,
    block_hashes: HashMap<u64, B256>,
    code: HashMap<B256, Bytecode>,
}

/// Exposes the same database through revm's synchronous shared-reference API.
#[derive(Debug)]
pub(crate) struct VmDatabase {
    db: RefCell<ForkDatabase>,
    runtime: Handle,
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

impl ForkDatabase {
    pub(crate) fn new(provider: DynProvider, header: &Header, limits: Limits) -> Self {
        let mut block_hashes = HashMap::new();
        if let Some(number) = header.number.checked_sub(1) {
            block_hashes.insert(number, header.parent_hash);
        }
        Self {
            provider,
            block: BlockId::hash_canonical(header.hash),
            budget: Arc::new(ReadBudget::new(limits)),
            accounts: HashMap::new(),
            storage: HashMap::new(),
            block_hashes,
            code: HashMap::new(),
        }
    }

    pub(crate) fn provider(&self) -> &DynProvider {
        &self.provider
    }

    pub(crate) fn block(&self) -> BlockId {
        self.block
    }

    pub(crate) fn budget(&self) -> &Arc<ReadBudget> {
        &self.budget
    }

    pub(crate) async fn account(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountInfo>, StateError> {
        if let Some(account) = self.accounts.get(&address) {
            return Ok(account.clone());
        }
        self.budget.record_state_read()?;
        let (balance, nonce, code) = tokio::try_join!(
            rpc_request(
                "eth_getBalance",
                self.provider.get_balance(address).block_id(self.block),
            ),
            rpc_request(
                "eth_getTransactionCount",
                self.provider
                    .get_transaction_count(address)
                    .block_id(self.block),
            ),
            rpc_request(
                "eth_getCode",
                self.provider.get_code_at(address).block_id(self.block),
            ),
        )?;
        let code = Bytecode::new_raw(code);
        let code_hash = code.hash_slow();
        let account = AccountInfo::new(balance, nonce, code_hash, code.clone());
        // Since EIP-161 the VM treats an empty account as missing. Earlier
        // blocks are rejected when selecting the execution context.
        let account = (!account.is_empty()).then_some(account);
        self.code.insert(code_hash, code);
        self.accounts.insert(address, account.clone());
        Ok(account)
    }

    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, StateError> {
        self.code
            .get(&code_hash)
            .cloned()
            .ok_or(StateError::CodeNotLoaded(code_hash))
    }

    async fn storage(&mut self, address: Address, index: U256) -> Result<U256, StateError> {
        if let Some(value) = self.storage.get(&(address, index)) {
            return Ok(*value);
        }
        self.budget.record_state_read()?;
        let value = rpc_request(
            "eth_getStorageAt",
            self.provider
                .get_storage_at(address, index)
                .block_id(self.block),
        )
        .await?;
        self.storage.insert((address, index), value);
        Ok(value)
    }

    async fn block_hash(&mut self, number: u64) -> Result<B256, StateError> {
        if let Some(hash) = self.block_hashes.get(&number) {
            return Ok(*hash);
        }
        // Query history at the fixed block; if unavailable, follow parent
        // hashes. A lookup by block number could silently cross a reorg.
        let slot = U256::from(number % HISTORY_SERVE_WINDOW as u64);
        let mut hash = B256::from(self.storage(HISTORY_STORAGE_ADDRESS, slot).await?);
        if hash.is_zero() {
            hash = self.ancestor_hash(number).await?;
        }
        self.block_hashes.insert(number, hash);
        Ok(hash)
    }

    async fn ancestor_hash(&mut self, number: u64) -> Result<B256, StateError> {
        let (mut child, mut hash) = (number + 1..=number + 256)
            .find_map(|child| Some((child, *self.block_hashes.get(&child)?)))
            .ok_or(StateError::BlockUnavailable(number))?;
        while child > number {
            self.budget.record_state_read()?;
            hash = rpc_request("eth_getBlockByHash", self.provider.get_block_by_hash(hash))
                .await?
                .ok_or(StateError::BlockUnavailable(child))?
                .header
                .parent_hash;
            child -= 1;
            self.block_hashes.insert(child, hash);
        }
        Ok(hash)
    }
}

async fn rpc_request<T>(
    operation: &'static str,
    request: impl IntoFuture<Output = Result<T, TransportError>>,
) -> Result<T, StateError> {
    request
        .into_future()
        .await
        .map_err(|source| StateError::Provider { operation, source })
}

impl VmDatabase {
    pub(crate) fn new(db: ForkDatabase, runtime: Handle) -> Self {
        Self {
            db: RefCell::new(db),
            runtime,
        }
    }
}

impl DatabaseRef for VmDatabase {
    type Error = StateError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.runtime.block_on(self.db.borrow_mut().account(address))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.db.borrow().code_by_hash(code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.runtime
            .block_on(self.db.borrow_mut().storage(address, index))
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.runtime
            .block_on(self.db.borrow_mut().block_hash(number))
    }
}

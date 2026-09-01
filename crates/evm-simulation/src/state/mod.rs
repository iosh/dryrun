use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use alloy::{
    eips::BlockId,
    network::Ethereum,
    primitives::{Address, B256, Bytes, TxKind, U256},
    providers::DynProvider,
};
use revm::{
    Context, ExecuteCommitEvm, ExecuteEvm, MainBuilder, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        JournalTr,
        result::{EVMError, ExecutionResult},
    },
    database::{AlloyDB, AlloyDBError, Cache, CacheDB, WrapDatabaseAsync},
    handler::EvmTr,
    state::EvmState,
};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::{EvmSimulationLimits, EvmStateAccessError};

pub(crate) type EvmDatabase = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider<Ethereum>>>>;
pub(crate) type MainnetEvm<INSP = ()> =
    revm::MainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, EvmDatabase>, INSP>;

#[derive(Debug, Clone)]
pub(crate) struct EvmStateSource {
    provider: DynProvider<Ethereum>,
    runtime_handle: Handle,
    block_hash: B256,
}

impl EvmStateSource {
    pub(crate) fn new(
        provider: DynProvider<Ethereum>,
        runtime_handle: Handle,
        block_hash: B256,
    ) -> Self {
        Self {
            provider,
            runtime_handle,
            block_hash,
        }
    }

    fn create_database(&self, cache: Cache) -> EvmDatabase {
        let block_id = BlockId::Hash(self.block_hash.into());
        let database = AlloyDB::new(self.provider.clone(), block_id);
        let database = WrapDatabaseAsync::with_handle(database, self.runtime_handle.clone());

        EvmDatabase {
            cache,
            db: database,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EvmStateViewFactory {
    source: EvmStateSource,
    cfg: CfgEnv,
    block: BlockEnv,
    limits: EvmSimulationLimits,
}

impl EvmStateViewFactory {
    pub(crate) fn new(source: EvmStateSource, cfg: CfgEnv, block: BlockEnv) -> Self {
        Self::with_limits(source, cfg, block, EvmSimulationLimits::default())
    }

    pub(crate) fn with_limits(
        source: EvmStateSource,
        cfg: CfgEnv,
        block: BlockEnv,
        limits: EvmSimulationLimits,
    ) -> Self {
        Self {
            source,
            cfg,
            block,
            limits,
        }
    }

    pub(crate) fn create_execution_evm<INSP>(&self, inspector: INSP) -> MainnetEvm<INSP> {
        self.create_evm(inspector, Cache::default(), &EvmState::default(), false)
    }

    fn create_evm<INSP>(
        &self,
        inspector: INSP,
        cache: Cache,
        overlay: &EvmState,
        read_call: bool,
    ) -> MainnetEvm<INSP> {
        let mut cfg = self.cfg.clone();
        if read_call {
            cfg.tx_chain_id_check = false;
            cfg.disable_nonce_check = true;
            cfg.disable_balance_check = true;
            cfg.disable_eip3607 = true;
            cfg.disable_base_fee = true;
        }

        let database = self.source.create_database(cache);
        let mut evm = Context::mainnet()
            .with_db(database)
            .modify_cfg_chained(|current| *current = cfg)
            .modify_block_chained(|current| *current = self.block.clone())
            .build_mainnet_with_inspector(inspector);
        evm.commit(overlay.clone());
        evm
    }
}

#[derive(Debug)]
pub(crate) struct EvmExecutionIdentity;

#[derive(Clone)]
pub struct EvmOccurrenceHandle {
    identity: Arc<EvmExecutionIdentity>,
    checkpoint_index: usize,
}

impl EvmOccurrenceHandle {
    pub(crate) fn new(identity: Arc<EvmExecutionIdentity>, checkpoint_index: usize) -> Self {
        Self {
            identity,
            checkpoint_index,
        }
    }
}

impl fmt::Debug for EvmOccurrenceHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EvmOccurrenceHandle")
            .field("checkpoint_index", &self.checkpoint_index)
            .finish_non_exhaustive()
    }
}

impl PartialEq for EvmOccurrenceHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
            && self.checkpoint_index == other.checkpoint_index
    }
}

impl Eq for EvmOccurrenceHandle {}

#[derive(Debug)]
pub struct EvmStateView {
    seed: EvmStateViewSeed,
    cache: RefCell<EvmStateViewCache>,
}

#[derive(Debug)]
pub struct EvmStateViews {
    identity: Arc<EvmExecutionIdentity>,
    initial: EvmStateView,
    occurrences: Vec<EvmStateView>,
    finalized: EvmStateView,
}

impl EvmStateViews {
    pub(crate) fn new(
        factory: EvmStateViewFactory,
        anchor_cache: Cache,
        identity: Arc<EvmExecutionIdentity>,
        caller: Address,
        occurrence_states: Vec<EvmState>,
        finalized_state: EvmState,
    ) -> Self {
        let anchor_cache = Arc::new(anchor_cache);
        let budget = Arc::new(EvmStateReadBudget::new(&factory.limits));
        let view = |overlay| {
            EvmStateView::new(EvmStateViewSeed {
                factory: factory.clone(),
                anchor_cache: Arc::clone(&anchor_cache),
                overlay,
                caller,
                budget: Arc::clone(&budget),
            })
        };

        Self {
            identity,
            initial: view(EvmState::default()),
            occurrences: occurrence_states.into_iter().map(view).collect(),
            finalized: view(finalized_state),
        }
    }

    pub const fn initial(&self) -> &EvmStateView {
        &self.initial
    }

    pub const fn finalized(&self) -> &EvmStateView {
        &self.finalized
    }

    pub const fn before(&self) -> &EvmStateView {
        self.initial()
    }

    pub const fn after(&self) -> &EvmStateView {
        self.finalized()
    }

    pub fn at(&self, occurrence: &EvmOccurrenceHandle) -> Result<&EvmStateView, EvmStateReadError> {
        let checkpoint_index = self.checkpoint_index(occurrence)?;
        Ok(self.occurrence(checkpoint_index))
    }

    pub fn around(
        &self,
        occurrence: &EvmOccurrenceHandle,
    ) -> Result<EvmOccurrenceStateViews<'_>, EvmStateReadError> {
        let checkpoint_index = self.checkpoint_index(occurrence)?;
        let current = self.occurrence(checkpoint_index);
        let previous = if checkpoint_index == 0 {
            &self.initial
        } else {
            self.occurrence(checkpoint_index - 1)
        };

        Ok(EvmOccurrenceStateViews { previous, current })
    }

    fn checkpoint_index(
        &self,
        occurrence: &EvmOccurrenceHandle,
    ) -> Result<usize, EvmStateReadError> {
        if !Arc::ptr_eq(&self.identity, &occurrence.identity) {
            return Err(EvmStateReadError::ForeignOccurrence);
        }
        Ok(occurrence.checkpoint_index)
    }

    fn occurrence(&self, checkpoint_index: usize) -> &EvmStateView {
        self.occurrences.get(checkpoint_index).unwrap_or_else(|| {
            unreachable!(
                "occurrence handle checkpoint must have a corresponding finalized state view"
            )
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EvmOccurrenceStateViews<'a> {
    previous: &'a EvmStateView,
    current: &'a EvmStateView,
}

impl<'a> EvmOccurrenceStateViews<'a> {
    pub const fn previous(self) -> &'a EvmStateView {
        self.previous
    }

    pub const fn current(self) -> &'a EvmStateView {
        self.current
    }
}

impl EvmStateView {
    fn new(seed: EvmStateViewSeed) -> Self {
        Self {
            seed,
            cache: RefCell::new(EvmStateViewCache::default()),
        }
    }

    pub fn read_account(&self, address: Address) -> Result<EvmAccountState, EvmStateReadError> {
        if let Some(account) = self.cache.borrow().accounts.get(&address) {
            return Ok(*account);
        }
        self.seed.budget.consume_state_read()?;
        let mut evm = self.create_read_evm();
        let account = evm
            .ctx_mut()
            .journaled_state
            .load_account_with_code(address)
            .map_err(EvmStateAccessError::from)
            .map_err(EvmStateReadError::from)?;
        let info = &account.info;
        let delegation = info.code.as_ref().and_then(|code| code.eip7702_address());
        let account = EvmAccountState {
            balance: info.balance,
            nonce: info.nonce,
            delegation,
        };
        self.cache.borrow_mut().accounts.insert(address, account);
        Ok(account)
    }

    pub fn storage_word(&self, contract: Address, slot: B256) -> Result<B256, EvmStateReadError> {
        if let Some(value) = self.cache.borrow().storage.get(&(contract, slot)) {
            return Ok(*value);
        }
        self.seed.budget.consume_state_read()?;
        let mut evm = self.create_read_evm();
        let storage_key = U256::from_be_slice(slot.as_slice());
        let journal = &mut evm.ctx_mut().journaled_state;
        journal
            .load_account_with_code(contract)
            .map_err(EvmStateAccessError::from)
            .map_err(EvmStateReadError::from)?;
        let value = journal
            .sload(contract, storage_key)
            .map_err(EvmStateAccessError::from)
            .map_err(EvmStateReadError::from)?
            .data;
        let value = B256::from(value.to_be_bytes::<32>());
        self.cache
            .borrow_mut()
            .storage
            .insert((contract, slot), value);
        Ok(value)
    }

    pub fn read_call(
        &self,
        target: Address,
        calldata: Bytes,
    ) -> Result<EvmReadCallOutcome, EvmStateReadError> {
        self.seed.budget.consume_read_call()?;
        let mut evm = self.seed.factory.create_evm(
            (),
            (*self.seed.anchor_cache).clone(),
            &self.seed.overlay,
            true,
        );
        let gas_limit = self
            .seed
            .factory
            .limits
            .read_call_gas_limit
            .unwrap_or(self.seed.factory.block.gas_limit)
            .min(self.seed.factory.block.gas_limit);
        let transaction = TxEnv {
            caller: self.seed.caller,
            gas_limit,
            kind: TxKind::Call(target),
            data: calldata,
            chain_id: None,
            ..Default::default()
        };
        let result = evm.transact_one(transaction).map_err(map_read_call_error)?;
        let outcome = match result {
            ExecutionResult::Success { output, .. } => {
                EvmReadCallOutcome::Success(output.into_data())
            }
            ExecutionResult::Revert { output, .. } => EvmReadCallOutcome::Reverted(output),
            ExecutionResult::Halt { reason, .. } => EvmReadCallOutcome::Halted {
                reason: reason.to_string(),
            },
        };
        if let Some(limit) = self.seed.factory.limits.max_read_call_output_bytes
            && outcome.output_len() > limit
        {
            return Err(EvmStateReadError::ReadCallOutputLimitExceeded { limit });
        }
        Ok(outcome)
    }

    fn create_read_evm(&self) -> MainnetEvm {
        self.seed.factory.create_evm(
            (),
            (*self.seed.anchor_cache).clone(),
            &self.seed.overlay,
            false,
        )
    }
}

#[derive(Debug)]
struct EvmStateViewSeed {
    factory: EvmStateViewFactory,
    anchor_cache: Arc<Cache>,
    overlay: EvmState,
    caller: Address,
    budget: Arc<EvmStateReadBudget>,
}

#[derive(Debug)]
struct EvmStateReadBudget {
    state_reads: AtomicUsize,
    read_calls: AtomicUsize,
    max_state_reads: Option<usize>,
    max_read_calls: Option<usize>,
}

impl EvmStateReadBudget {
    fn new(limits: &EvmSimulationLimits) -> Self {
        Self {
            state_reads: AtomicUsize::new(0),
            read_calls: AtomicUsize::new(0),
            max_state_reads: limits.max_state_reads,
            max_read_calls: limits.max_read_calls,
        }
    }

    fn consume_state_read(&self) -> Result<(), EvmStateReadError> {
        let Some(limit) = self.max_state_reads else {
            return Ok(());
        };
        self.state_reads
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used < limit).then_some(used + 1)
            })
            .map(|_| ())
            .map_err(|_| EvmStateReadError::StateReadLimitExceeded { limit })
    }

    fn consume_read_call(&self) -> Result<(), EvmStateReadError> {
        let Some(limit) = self.max_read_calls else {
            return Ok(());
        };
        self.read_calls
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used < limit).then_some(used + 1)
            })
            .map(|_| ())
            .map_err(|_| EvmStateReadError::ReadCallLimitExceeded { limit })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvmAccountState {
    balance: U256,
    nonce: u64,
    delegation: Option<Address>,
}

impl EvmAccountState {
    pub const fn balance(&self) -> U256 {
        self.balance
    }

    pub const fn nonce(&self) -> u64 {
        self.nonce
    }

    pub const fn delegation(&self) -> Option<Address> {
        self.delegation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Halted { reason: String },
}

impl EvmReadCallOutcome {
    pub fn output(&self) -> Option<&Bytes> {
        match self {
            Self::Success(output) | Self::Reverted(output) => Some(output),
            Self::Halted { .. } => None,
        }
    }

    fn output_len(&self) -> usize {
        self.output().map_or(0, |output| output.len())
    }
}

#[derive(Debug, Default)]
struct EvmStateViewCache {
    accounts: HashMap<Address, EvmAccountState>,
    storage: HashMap<(Address, B256), B256>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EvmStateAccessError),

    #[error("occurrence handle belongs to another EVM execution")]
    ForeignOccurrence,

    #[error("state-read limit {limit} exceeded")]
    StateReadLimitExceeded { limit: usize },

    #[error("read-call limit {limit} exceeded")]
    ReadCallLimitExceeded { limit: usize },

    #[error("read-call output limit {limit} bytes exceeded")]
    ReadCallOutputLimitExceeded { limit: usize },

    #[error("read call failed: {details}")]
    ReadCallFailed { details: String },
}

fn map_read_call_error(error: EVMError<AlloyDBError>) -> EvmStateReadError {
    match error {
        EVMError::Transaction(error) => EvmStateReadError::ReadCallFailed {
            details: error.to_string(),
        },
        EVMError::Header(error) => EvmStateReadError::ReadCallFailed {
            details: error.to_string(),
        },
        EVMError::Database(error) => {
            EvmStateReadError::StateAccess(EvmStateAccessError::from(error))
        }
        EVMError::Custom(details) => EvmStateReadError::ReadCallFailed { details },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::{
        network::Ethereum,
        primitives::{Address, B256, Bytes, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };
    use revm::{
        context::{BlockEnv, CfgEnv},
        primitives::hardfork::SpecId,
        state::{Account, AccountInfo, Bytecode, EvmState, EvmStorageSlot},
    };

    use super::{
        EvmExecutionIdentity, EvmOccurrenceHandle, EvmReadCallOutcome, EvmStateReadError,
        EvmStateSource, EvmStateViewFactory, EvmStateViews,
    };

    #[test]
    fn occurrence_handles_read_the_ordered_state_chain() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");
        let account = Address::repeat_byte(1);
        let caller = Address::repeat_byte(2);
        let (factory, anchor_cache) = factory_and_cache(
            runtime.handle().clone(),
            account,
            caller,
            Bytecode::default(),
        );
        let identity = Arc::new(EvmExecutionIdentity);
        let views = EvmStateViews::new(
            factory,
            anchor_cache,
            Arc::clone(&identity),
            caller,
            vec![storage_state(account, 100), storage_state(account, 0)],
            storage_state(account, 0),
        );
        let first = EvmOccurrenceHandle::new(Arc::clone(&identity), 0);
        let second = EvmOccurrenceHandle::new(Arc::clone(&identity), 1);

        assert_eq!(storage_value(views.initial(), account), U256::ZERO);
        let first_views = views.around(&first).expect("first handle should resolve");
        assert_eq!(storage_value(first_views.previous(), account), U256::ZERO);
        assert_eq!(
            storage_value(first_views.current(), account),
            U256::from(100)
        );
        let second_views = views.around(&second).expect("second handle should resolve");
        assert_eq!(
            storage_value(second_views.previous(), account),
            U256::from(100)
        );
        assert_eq!(storage_value(second_views.current(), account), U256::ZERO);
        assert_eq!(storage_value(views.finalized(), account), U256::ZERO);

        let foreign = EvmOccurrenceHandle::new(Arc::new(EvmExecutionIdentity), 0);
        assert!(matches!(
            views.at(&foreign),
            Err(EvmStateReadError::ForeignOccurrence)
        ));
    }

    #[test]
    fn read_calls_discard_all_state_writes() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");
        let contract = Address::repeat_byte(1);
        let caller = Address::repeat_byte(2);
        let code = Bytecode::new_raw(Bytes::from(vec![
            0x5f, // PUSH0
            0x54, // SLOAD
            0x60, 0x01, // PUSH1 1
            0x01, // ADD
            0x80, // DUP1
            0x5f, // PUSH0
            0x55, // SSTORE
            0x5f, // PUSH0
            0x52, // MSTORE
            0x60, 0x20, // PUSH1 32
            0x5f, // PUSH0
            0xf3, // RETURN
        ]));
        let (factory, anchor_cache) =
            factory_and_cache(runtime.handle().clone(), contract, caller, code);
        let identity = Arc::new(EvmExecutionIdentity);
        let views = EvmStateViews::new(
            factory,
            anchor_cache,
            identity,
            caller,
            Vec::new(),
            EvmState::default(),
        );

        let EvmReadCallOutcome::Success(output) = views
            .initial()
            .read_call(contract, Bytes::new())
            .expect("read call should succeed")
        else {
            panic!("read call should return successfully");
        };
        assert_eq!(U256::from_be_slice(&output), U256::from(1));
        assert_eq!(storage_value(views.initial(), contract), U256::ZERO);
    }

    fn factory_and_cache(
        runtime_handle: tokio::runtime::Handle,
        contract: Address,
        caller: Address,
        code: Bytecode,
    ) -> (EvmStateViewFactory, revm::database::Cache) {
        let source = EvmStateSource::new(mock_provider(), runtime_handle, B256::repeat_byte(3));
        let mut database = source.create_database(revm::database::Cache::default());
        database.insert_account_info(contract, AccountInfo::default().with_code(code));
        database.insert_account_info(caller, AccountInfo::default());
        database
            .insert_account_storage(contract, U256::ZERO, U256::ZERO)
            .expect("cached account should accept storage");
        let block = BlockEnv {
            beneficiary: caller,
            gas_limit: 30_000_000,
            ..Default::default()
        };
        let cfg = CfgEnv::new_with_spec(SpecId::OSAKA).with_chain_id(1);

        (EvmStateViewFactory::new(source, cfg, block), database.cache)
    }

    fn storage_state(account: Address, value: u64) -> EvmState {
        let account_state = Account::from(AccountInfo::default())
            .with_storage(std::iter::once((
                U256::ZERO,
                EvmStorageSlot::new_changed(U256::ZERO, U256::from(value), 0),
            )))
            .with_touched_mark();
        std::iter::once((account, account_state)).collect()
    }

    fn storage_value(view: &super::EvmStateView, account: Address) -> U256 {
        let word = view
            .storage_word(account, B256::ZERO)
            .expect("storage should be readable");
        U256::from_be_slice(word.as_slice())
    }

    fn mock_provider() -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(Asserter::new())).erased()
    }
}

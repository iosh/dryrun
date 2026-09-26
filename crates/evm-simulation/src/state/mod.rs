use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

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
use simulation_core::observation::{AnalysisLimitExceeded, ReadBudget};
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
        let block_id = BlockId::hash_canonical(self.block_hash);
        let database = AlloyDB::new(self.provider.clone(), block_id);
        let database = WrapDatabaseAsync::with_handle(database, self.runtime_handle.clone());

        EvmDatabase {
            cache,
            db: database,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EvmStateAccessFactory {
    source: EvmStateSource,
    cfg: CfgEnv,
    block: BlockEnv,
    limits: EvmSimulationLimits,
}

impl EvmStateAccessFactory {
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
pub struct EvmStateReader {
    seed: EvmStateReaderSeed,
    cache: RefCell<EvmStateReaderCache>,
}

#[derive(Debug)]
pub struct EvmStateAccess {
    initial: EvmStateReader,
    checkpoints: Vec<(usize, EvmStateReader)>,
    finalized: EvmStateReader,
}

impl EvmStateAccess {
    pub(crate) fn new(
        factory: EvmStateAccessFactory,
        anchor_cache: Cache,
        caller: Address,
        checkpoints: Vec<(usize, EvmState)>,
        finalized_state: EvmState,
    ) -> Self {
        let anchor_cache = Arc::new(anchor_cache);
        let budget = Rc::new(ReadBudget::new(factory.limits));
        let view = |overlay| {
            EvmStateReader::new(EvmStateReaderSeed {
                factory: factory.clone(),
                anchor_cache: Arc::clone(&anchor_cache),
                overlay,
                caller,
                budget: Rc::clone(&budget),
            })
        };

        Self {
            initial: view(EvmState::default()),
            checkpoints: checkpoints
                .into_iter()
                .map(|(index, state)| (index, view(state)))
                .collect(),
            finalized: view(finalized_state),
        }
    }

    pub const fn initial(&self) -> &EvmStateReader {
        &self.initial
    }

    pub const fn finalized(&self) -> &EvmStateReader {
        &self.finalized
    }

    pub(crate) fn log_checkpoints(
        &self,
    ) -> impl Iterator<Item = (usize, &EvmStateReader, &EvmStateReader)> {
        let mut previous = self.initial();
        self.checkpoints.iter().map(move |(log_index, current)| {
            let checkpoint = (*log_index, previous, current);
            previous = current;
            checkpoint
        })
    }
}

impl EvmStateReader {
    fn new(seed: EvmStateReaderSeed) -> Self {
        Self {
            seed,
            cache: RefCell::new(EvmStateReaderCache::default()),
        }
    }

    pub fn read_account(&self, address: Address) -> Result<EvmAccountState, EvmStateReadError> {
        if let Some(account) = self.cache.borrow().accounts.get(&address) {
            return Ok(*account);
        }
        self.seed.budget.state_read()?;
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

    pub fn code(&self, address: Address) -> Result<Bytes, EvmStateReadError> {
        if let Some(code) = self.cache.borrow().code.get(&address) {
            return Ok(code.clone());
        }
        self.seed.budget.state_read()?;
        let mut evm = self.create_read_evm();
        let account = evm
            .ctx_mut()
            .journaled_state
            .load_account_with_code(address)
            .map_err(EvmStateAccessError::from)?;
        let code = account
            .info
            .code
            .as_ref()
            .map(|code| code.original_bytes())
            .unwrap_or_default();
        self.cache.borrow_mut().code.insert(address, code.clone());
        Ok(code)
    }

    pub fn storage_word(&self, contract: Address, slot: B256) -> Result<B256, EvmStateReadError> {
        if let Some(value) = self.cache.borrow().storage.get(&(contract, slot)) {
            return Ok(*value);
        }
        self.seed.budget.state_read()?;
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
        if let Some(outcome) = self
            .cache
            .borrow()
            .read_calls
            .get(&(target, calldata.clone()))
        {
            return Ok(outcome.clone());
        }
        self.seed.budget.read_call()?;
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
            .min(self.seed.factory.block.gas_limit);
        let transaction = TxEnv {
            caller: self.seed.caller,
            gas_limit,
            kind: TxKind::Call(target),
            data: calldata.clone(),
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
        self.seed.budget.check_output(outcome.output_len())?;
        self.cache
            .borrow_mut()
            .read_calls
            .insert((target, calldata), outcome.clone());
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
struct EvmStateReaderSeed {
    factory: EvmStateAccessFactory,
    anchor_cache: Arc<Cache>,
    overlay: EvmState,
    caller: Address,
    budget: Rc<ReadBudget>,
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
struct EvmStateReaderCache {
    accounts: HashMap<Address, EvmAccountState>,
    storage: HashMap<(Address, B256), B256>,
    code: HashMap<Address, Bytes>,
    read_calls: HashMap<(Address, Bytes), EvmReadCallOutcome>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvmStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EvmStateAccessError),

    #[error(transparent)]
    LimitExceeded(#[from] AnalysisLimitExceeded),

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

    use crate::EvmSimulationLimits;

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

    use super::{EvmReadCallOutcome, EvmStateAccess, EvmStateAccessFactory, EvmStateSource};

    #[test]
    fn checkpoints_read_the_ordered_state_chain() {
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
        let access = EvmStateAccess::new(
            factory,
            anchor_cache,
            caller,
            vec![
                (0, storage_state(account, 100)),
                (1, storage_state(account, 0)),
            ],
            storage_state(account, 0),
        );
        let checkpoints = access.log_checkpoints().collect::<Vec<_>>();
        assert_eq!(storage_value(access.initial(), account), U256::ZERO);
        assert_eq!(storage_value(checkpoints[0].1, account), U256::ZERO);
        assert_eq!(storage_value(checkpoints[0].2, account), U256::from(100));
        assert_eq!(storage_value(checkpoints[1].1, account), U256::from(100));
        assert_eq!(storage_value(checkpoints[1].2, account), U256::ZERO);
        assert_eq!(storage_value(access.finalized(), account), U256::ZERO);
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
        let access = EvmStateAccess::new(
            factory,
            anchor_cache,
            caller,
            Vec::new(),
            EvmState::default(),
        );

        let EvmReadCallOutcome::Success(output) = access
            .initial()
            .read_call(contract, Bytes::new())
            .expect("read call should succeed")
        else {
            panic!("read call should return successfully");
        };
        assert_eq!(U256::from_be_slice(&output), U256::from(1));
        assert_eq!(storage_value(access.initial(), contract), U256::ZERO);
    }

    fn factory_and_cache(
        runtime_handle: tokio::runtime::Handle,
        contract: Address,
        caller: Address,
        code: Bytecode,
    ) -> (EvmStateAccessFactory, revm::database::Cache) {
        let source = EvmStateSource::new(mock_provider(), runtime_handle, B256::repeat_byte(3));
        let mut database = source.create_database(revm::database::Cache::default());
        database.insert_account_info(contract, AccountInfo::default().with_code(code));
        database.insert_account_info(caller, AccountInfo::default());
        database
            .insert_account_storage(contract, U256::ZERO, U256::ZERO)
            .expect("cached account should accept storage");
        let block = BlockEnv {
            beneficiary: caller,
            // Keep the fixture below revm's transaction gas cap so the
            // isolated read-call exercises state rollback rather than input
            // rejection.
            gas_limit: 10_000_000,
            ..Default::default()
        };
        let cfg = CfgEnv::new_with_spec(SpecId::OSAKA).with_chain_id(1);

        (
            EvmStateAccessFactory::with_limits(source, cfg, block, EvmSimulationLimits::default()),
            database.cache,
        )
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

    fn storage_value(reader: &super::EvmStateReader, account: Address) -> U256 {
        let word = reader
            .storage_word(account, B256::ZERO)
            .expect("storage should be readable");
        U256::from_be_slice(word.as_slice())
    }

    fn mock_provider() -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(Asserter::new())).erased()
    }
}

impl contract_standards::MetadataReader<Address> for EvmStateReader {
    type Error = EvmStateReadError;
    fn metadata_call(&self, address: &Address, input: Bytes) -> Result<Option<Bytes>, Self::Error> {
        Ok(match self.read_call(*address, input)? {
            EvmReadCallOutcome::Success(output) => Some(output),
            EvmReadCallOutcome::Reverted(_) | EvmReadCallOutcome::Halted { .. } => None,
        })
    }
}

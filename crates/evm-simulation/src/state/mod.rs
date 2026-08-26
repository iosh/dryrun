mod read_call;

use std::{cell::RefCell, collections::HashMap};

use alloy::{
    eips::BlockId,
    network::Ethereum,
    primitives::{Address, B256, Bytes},
    providers::DynProvider,
};
use revm::{
    Context, MainnetEvm as RevmMainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::result::HaltReason,
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    handler::EvmTr,
};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::{CompleteTransaction, EvmStateAccessError};

use self::read_call::execute_read_call;

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
pub(crate) struct EvmStateView {
    inner: RefCell<EvmStateViewInner>,
    call_context: EvmReadCallContext,
}

impl EvmStateView {
    pub(crate) fn from_execution<INSP>(
        evm: MainnetEvm<INSP>,
        transaction: &CompleteTransaction,
    ) -> Self {
        let mut evm = evm.with_inspector(());
        let call_context = EvmReadCallContext {
            caller: transaction.from,
            nonce: transaction.nonce.saturating_add(1),
            chain_id: transaction.chain_id,
        };
        let cfg = &mut evm.ctx_mut().cfg;
        cfg.disable_nonce_check = true;
        cfg.disable_balance_check = true;
        cfg.disable_eip3607 = true;
        cfg.disable_base_fee = true;
        cfg.disable_fee_charge = true;

        Self {
            inner: RefCell::new(EvmStateViewInner::new(evm)),
            call_context,
        }
    }

    pub(crate) fn read_call(
        &self,
        target: Address,
        data: Bytes,
    ) -> Result<EvmReadCallResult, EvmStateReadError> {
        let key = (target, data.clone());
        if let Some(result) = self.inner.borrow().read_calls.get(&key) {
            return Ok(result.clone());
        }
        let mut inner = self.inner.borrow_mut();
        let result = execute_read_call(&mut inner.evm, self.call_context, target, data)?;
        inner.read_calls.insert(key, result.clone());
        Ok(result)
    }
}

#[derive(Debug)]
struct EvmStateViewInner {
    evm: MainnetEvm,
    read_calls: HashMap<(Address, Bytes), EvmReadCallResult>,
}

impl EvmStateViewInner {
    fn new(evm: MainnetEvm) -> Self {
        Self {
            evm,
            read_calls: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvmReadCallResult {
    Success(Bytes),
    Reverted(Bytes),
    Halted(HaltReason),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EvmReadCallContext {
    caller: Address,
    nonce: u64,
    chain_id: u64,
}

#[derive(Debug, Error)]
pub(crate) enum EvmStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EvmStateAccessError),

    #[error("read call execution failed: {details}")]
    ReadCallExecution { details: String },
}

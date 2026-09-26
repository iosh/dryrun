use crate::execution::{IsolatedReadCallError, ReadCallInput, execute_isolated_read_call};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    rc::Rc,
    sync::Arc,
};

use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_executor::{
    machine::Machine,
    state::{SavedState, State},
};
use cfx_types::AddressSpaceUtil;
use cfx_vm_types::{Env, Spec};
use simulation_core::observation::{AnalysisLimitExceeded, ReadBudget};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::{
    execution::{PreparedTransactionExecution, build_conflux_state},
    primitive::{address_to_cfx, b256_to_cfx, u256_from_cfx},
    state::ConfluxStateSource,
};

use super::EspaceStateAccessError;

pub use simulation_core::observation::AnalysisLimits as EspaceSimulationLimits;

/// Initial, committed log and finalized states over one fixed RPC anchor.
pub struct EspaceStateAccess {
    source: Arc<ConfluxStateSource>,
    runtime_handle: Handle,
    initial: EspaceStateReader,
    checkpoints: Vec<(usize, EspaceStateReader)>,
    finalized: EspaceStateReader,
}

impl fmt::Debug for EspaceStateAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EspaceStateAccess")
            .finish_non_exhaustive()
    }
}

impl EspaceStateAccess {
    pub(crate) fn new(
        source: Arc<ConfluxStateSource>,
        runtime_handle: Handle,
        finalized_state: State,
        machine: Arc<Machine>,
        prepared: &PreparedTransactionExecution,
        caller: Address,
        limits: EspaceSimulationLimits,
    ) -> Result<Self, EspaceStateAccessError> {
        let initial_state = build_conflux_state(Arc::clone(&source), runtime_handle.clone())
            .map_err(|source| EspaceStateAccessError::Initialization { source })?;
        let context = Rc::new(EspaceReadContext {
            machine,
            env: prepared.env.clone(),
            spec: prepared.spec.clone(),
            caller,
            budget: ReadBudget::new(limits),
            analysis_started: Cell::new(false),
        });

        Ok(Self {
            source,
            runtime_handle,
            initial: EspaceStateReader::new(initial_state, Rc::clone(&context)),
            checkpoints: Vec::new(),
            finalized: EspaceStateReader::new(finalized_state, context),
        })
    }

    pub(crate) fn restore_checkpoint(
        &mut self,
        log_index: usize,
        snapshot: SavedState,
    ) -> Result<(), EspaceStateAccessError> {
        let mut state = build_conflux_state(Arc::clone(&self.source), self.runtime_handle.clone())
            .map_err(|source| EspaceStateAccessError::Initialization { source })?;
        state.restore(snapshot);
        self.checkpoints.push((
            log_index,
            EspaceStateReader::new(state, Rc::clone(&self.finalized.context)),
        ));
        Ok(())
    }

    pub(crate) fn start_analysis(&self) {
        self.finalized.context.analysis_started.set(true);
    }

    pub const fn initial(&self) -> &EspaceStateReader {
        &self.initial
    }

    pub(crate) fn caller(&self) -> Address {
        self.finalized.context.caller
    }

    pub const fn finalized(&self) -> &EspaceStateReader {
        &self.finalized
    }

    pub(crate) fn log_checkpoints(
        &self,
    ) -> impl Iterator<Item = (usize, &EspaceStateReader, &EspaceStateReader)> {
        let mut previous = self.initial();
        self.checkpoints.iter().map(move |(log_index, current)| {
            let checkpoint = (*log_index, previous, current);
            previous = current;
            checkpoint
        })
    }
}

struct EspaceReadContext {
    machine: Arc<Machine>,
    env: Env,
    spec: Spec,
    caller: Address,
    budget: ReadBudget,
    analysis_started: Cell<bool>,
}

/// A controlled reader over one fixed eSpace state point.
pub struct EspaceStateReader {
    state: RefCell<State>,
    context: Rc<EspaceReadContext>,
    poisoned: Cell<bool>,
    cache: RefCell<EspaceStateReaderCache>,
}

impl fmt::Debug for EspaceStateReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EspaceStateReader")
            .field("caller", &self.context.caller)
            .finish_non_exhaustive()
    }
}

impl EspaceStateReader {
    fn new(state: State, context: Rc<EspaceReadContext>) -> Self {
        Self {
            state: RefCell::new(state),
            context,
            poisoned: Cell::new(false),
            cache: RefCell::new(EspaceStateReaderCache::default()),
        }
    }

    pub fn read_account(
        &self,
        address: Address,
    ) -> Result<EspaceAccountState, EspaceStateReadError> {
        self.ensure_usable()?;
        if let Some(account) = self.cache.borrow().accounts.get(&address) {
            return Ok(account.clone());
        }
        if self.context.analysis_started.get() {
            self.context.budget.state_read()?;
        }

        let address = address_to_cfx(address).with_evm_space();
        let state = self.state.borrow();
        let exists = state
            .exists(&address)
            .map_err(|source| self.state_error("read eSpace account existence", source))?;
        let account = if exists {
            let balance = state
                .balance(&address)
                .map_err(|source| self.state_error("read eSpace account balance", source))?;
            let nonce = state
                .nonce(&address)
                .map_err(|source| self.state_error("read eSpace account nonce", source))?;
            let code = state
                .code(&address)
                .map_err(|source| self.state_error("read eSpace account code", source))?
                .map(|code| Bytes::copy_from_slice(code.as_slice()));
            EspaceAccountState {
                exists: true,
                balance: u256_from_cfx(balance),
                nonce: u256_from_cfx(nonce),
                code,
            }
        } else {
            EspaceAccountState {
                exists: false,
                balance: U256::ZERO,
                nonce: U256::ZERO,
                code: None,
            }
        };
        drop(state);
        self.cache
            .borrow_mut()
            .accounts
            .insert(address_to_alloy(address), account.clone());
        Ok(account)
    }

    pub fn native_balance(&self, address: Address) -> Result<U256, EspaceStateReadError> {
        self.read_account(address).map(|account| account.balance())
    }

    pub fn storage_word(
        &self,
        contract: Address,
        slot: B256,
    ) -> Result<B256, EspaceStateReadError> {
        self.ensure_usable()?;
        if let Some(value) = self.cache.borrow().storage.get(&(contract, slot)) {
            return Ok(*value);
        }
        if self.context.analysis_started.get() {
            self.context.budget.state_read()?;
        }

        let address = address_to_cfx(contract).with_evm_space();
        let key = b256_to_cfx(slot).as_bytes().to_vec();
        let state = self.state.borrow();
        let value = state
            .storage_at(&address, &key)
            .map_err(|source| self.state_error("read eSpace storage", source))?;
        let value = B256::from(u256_from_cfx(value).to_be_bytes::<32>());
        drop(state);
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
    ) -> Result<EspaceReadCallOutcome, EspaceStateReadError> {
        self.ensure_usable()?;
        if let Some(outcome) = self
            .cache
            .borrow()
            .read_calls
            .get(&(target, calldata.clone()))
        {
            return Ok(outcome.clone());
        }
        self.context.budget.read_call()?;

        let mut state = self.state.borrow_mut();
        let outcome = execute_isolated_read_call(
            &mut state,
            &self.context.machine,
            &self.context.env,
            &self.context.spec,
            ReadCallInput {
                sender: address_to_cfx(self.context.caller).with_evm_space(),
                target: address_to_cfx(target),
                data: calldata.clone(),
                gas_limit: self.context.budget.limits().read_call_gas_limit,
            },
        )
        .map_err(|error| match error {
            IsolatedReadCallError::StateAccess(source) => {
                self.state_error("execute eSpace read call", source)
            }
            IsolatedReadCallError::Execution(details) => {
                EspaceStateReadError::ReadCallFailed { details }
            }
        });
        drop(state);

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.poisoned.set(true);
                return Err(error);
            }
        };
        self.context
            .budget
            .check_output(outcome.output().map_or(0, |output| output.len()))?;
        self.cache
            .borrow_mut()
            .read_calls
            .insert((target, calldata), outcome.clone());
        Ok(outcome)
    }

    fn ensure_usable(&self) -> Result<(), EspaceStateReadError> {
        if self.poisoned.get() {
            Err(EspaceStateReadError::Poisoned)
        } else {
            Ok(())
        }
    }

    fn state_error(
        &self,
        operation: &'static str,
        source: cfx_statedb::Error,
    ) -> EspaceStateReadError {
        EspaceStateReadError::StateAccess(EspaceStateAccessError::Operation { operation, source })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceAccountState {
    exists: bool,
    balance: U256,
    nonce: U256,
    code: Option<Bytes>,
}

impl EspaceAccountState {
    pub const fn exists(&self) -> bool {
        self.exists
    }

    pub const fn balance(&self) -> U256 {
        self.balance
    }

    pub const fn nonce(&self) -> U256 {
        self.nonce
    }

    pub fn code(&self) -> Option<&Bytes> {
        self.code.as_ref()
    }

    /// Returns the EIP-7702 delegation target encoded in the account code.
    pub fn delegation(&self) -> Option<Address> {
        let code = self.code.as_ref()?;
        let payload = code.as_ref().strip_prefix(primitives::CODE_PREFIX_7702)?;
        (payload.len() == 20).then(|| Address::from_slice(payload))
    }
}

pub use crate::execution::ReadCallOutcome as EspaceReadCallOutcome;

#[derive(Debug, Default)]
struct EspaceStateReaderCache {
    accounts: HashMap<Address, EspaceAccountState>,
    storage: HashMap<(Address, B256), B256>,
    read_calls: HashMap<(Address, Bytes), EspaceReadCallOutcome>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EspaceStateAccessError),

    #[error(transparent)]
    LimitExceeded(#[from] AnalysisLimitExceeded),

    #[error("read call failed: {details}")]
    ReadCallFailed { details: String },

    #[error("state reader is unavailable after an isolated read-call failure")]
    Poisoned,
}

fn address_to_alloy(address: cfx_types::AddressWithSpace) -> Address {
    Address::from_slice(address.address.as_bytes())
}

#[cfg(test)]
mod tests;

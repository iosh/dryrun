use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_executor::{machine::Machine, state::State};
use cfx_types::AddressSpaceUtil;
use thiserror::Error;

use crate::{
    execution::PreparedTransactionExecution,
    primitive::{address_to_cfx, b256_to_cfx, u256_from_cfx},
};

use super::{
    EspaceChangesError, EspaceStateAccessError,
    changes::{IsolatedReadCallError, ReadCallOutcome, execute_isolated_read_call},
};

/// Resource limits enforced by eSpace state readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EspaceSimulationLimits {
    pub max_state_reads: usize,
    pub max_read_calls: usize,
    pub read_call_gas_limit: u64,
    pub max_read_call_output_bytes: usize,
}

impl EspaceSimulationLimits {
    pub const fn new(
        max_state_reads: usize,
        max_read_calls: usize,
        read_call_gas_limit: u64,
        max_read_call_output_bytes: usize,
    ) -> Self {
        Self {
            max_state_reads,
            max_read_calls,
            read_call_gas_limit,
            max_read_call_output_bytes,
        }
    }
}

/// Fixed initial and finalized state access for one execution.
pub struct EspaceStateAccess {
    initial: EspaceStateReader,
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
        initial_state: State,
        finalized_state: State,
        machine: Arc<Machine>,
        prepared: PreparedTransactionExecution,
        caller: Address,
        limits: EspaceSimulationLimits,
    ) -> Self {
        let budget = Arc::new(EspaceStateReadBudget::new(limits));
        let reader = |state| {
            EspaceStateReader::new(
                state,
                Arc::clone(&machine),
                prepared.clone(),
                caller,
                Arc::clone(&budget),
            )
        };

        Self {
            initial: reader(initial_state),
            finalized: reader(finalized_state),
        }
    }

    pub const fn initial(&self) -> &EspaceStateReader {
        &self.initial
    }

    pub const fn finalized(&self) -> &EspaceStateReader {
        &self.finalized
    }
}

/// A controlled reader over one fixed eSpace state point.
pub struct EspaceStateReader {
    state: RefCell<State>,
    machine: Arc<Machine>,
    prepared: PreparedTransactionExecution,
    caller: Address,
    budget: Arc<EspaceStateReadBudget>,
    poisoned: Cell<bool>,
    cache: RefCell<EspaceStateReaderCache>,
}

impl fmt::Debug for EspaceStateReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EspaceStateReader")
            .field("caller", &self.caller)
            .finish_non_exhaustive()
    }
}

impl EspaceStateReader {
    fn new(
        state: State,
        machine: Arc<Machine>,
        prepared: PreparedTransactionExecution,
        caller: Address,
        budget: Arc<EspaceStateReadBudget>,
    ) -> Self {
        Self {
            state: RefCell::new(state),
            machine,
            prepared,
            caller,
            budget,
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
        self.budget.consume_state_read()?;

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
        self.budget.consume_state_read()?;

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
        self.budget.consume_read_call()?;

        let mut state = self.state.borrow_mut();
        let outcome = execute_isolated_read_call(
            &mut state,
            &self.machine,
            &self.prepared.env,
            &self.prepared.spec,
            self.caller,
            target,
            calldata.clone(),
            Some(self.budget.limits.read_call_gas_limit),
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
            Ok(ReadCallOutcome::Success(output)) => EspaceReadCallOutcome::Success(output),
            Ok(ReadCallOutcome::Reverted(output)) => EspaceReadCallOutcome::Reverted(output),
            Ok(ReadCallOutcome::Failed) => EspaceReadCallOutcome::Failed,
            Err(error) => {
                self.poisoned.set(true);
                return Err(error);
            }
        };
        if outcome.output_len() > self.budget.limits.max_read_call_output_bytes {
            return Err(EspaceStateReadError::ReadCallOutputLimitExceeded {
                limit: self.budget.limits.max_read_call_output_bytes,
            });
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Failed,
}

impl EspaceReadCallOutcome {
    pub fn output(&self) -> Option<&Bytes> {
        match self {
            Self::Success(output) | Self::Reverted(output) => Some(output),
            Self::Failed => None,
        }
    }

    fn output_len(&self) -> usize {
        self.output().map_or(0, |output| output.len())
    }
}

#[derive(Debug, Default)]
struct EspaceStateReaderCache {
    accounts: HashMap<Address, EspaceAccountState>,
    storage: HashMap<(Address, B256), B256>,
    read_calls: HashMap<(Address, Bytes), EspaceReadCallOutcome>,
}

#[derive(Debug)]
struct EspaceStateReadBudget {
    state_reads: AtomicUsize,
    read_calls: AtomicUsize,
    limits: EspaceSimulationLimits,
}

impl EspaceStateReadBudget {
    fn new(limits: EspaceSimulationLimits) -> Self {
        Self {
            state_reads: AtomicUsize::new(0),
            read_calls: AtomicUsize::new(0),
            limits,
        }
    }

    fn consume_state_read(&self) -> Result<(), EspaceStateReadError> {
        self.state_reads
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used < self.limits.max_state_reads).then_some(used + 1)
            })
            .map(|_| ())
            .map_err(|_| EspaceStateReadError::StateReadLimitExceeded {
                limit: self.limits.max_state_reads,
            })
    }

    fn consume_read_call(&self) -> Result<(), EspaceStateReadError> {
        self.read_calls
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used < self.limits.max_read_calls).then_some(used + 1)
            })
            .map(|_| ())
            .map_err(|_| EspaceStateReadError::ReadCallLimitExceeded {
                limit: self.limits.max_read_calls,
            })
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceStateReadError {
    #[error(transparent)]
    StateAccess(#[from] EspaceStateAccessError),

    #[error("state-read limit {limit} exceeded")]
    StateReadLimitExceeded { limit: usize },

    #[error("read-call limit {limit} exceeded")]
    ReadCallLimitExceeded { limit: usize },

    #[error("read-call output limit {limit} bytes exceeded")]
    ReadCallOutputLimitExceeded { limit: usize },

    #[error("read call failed: {details}")]
    ReadCallFailed { details: String },

    #[error("state reader is unavailable after an isolated read-call failure")]
    Poisoned,
}

impl From<EspaceStateReadError> for EspaceChangesError {
    fn from(error: EspaceStateReadError) -> Self {
        Self::StateAccess {
            details: error.to_string(),
        }
    }
}

fn address_to_alloy(address: cfx_types::AddressWithSpace) -> Address {
    Address::from_slice(address.address.as_bytes())
}

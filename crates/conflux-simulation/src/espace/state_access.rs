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
use cfx_executor::{
    machine::Machine,
    state::{SavedState, State},
};
use cfx_types::{AddressSpaceUtil, Space};
use cfx_vm_types::{Env, Spec};
use thiserror::Error;
use tokio::runtime::Handle;

use crate::{
    execution::{PreparedTransactionExecution, build_conflux_state},
    primitive::{address_to_cfx, b256_to_cfx, u256_from_cfx},
    state::ConfluxStateSource,
};

use super::{
    EspaceChangesError, EspaceStateAccessError,
    changes::{IsolatedReadCallError, ReadCallOutcome, execute_isolated_read_call},
};

/// Resource limits enforced by eSpace state readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EspaceSimulationLimits {
    pub max_occurrence_checkpoints: usize,
    pub max_state_reads: usize,
    pub max_read_calls: usize,
    pub read_call_gas_limit: u64,
    pub max_read_call_output_bytes: usize,
}

impl EspaceSimulationLimits {
    pub const fn new(
        max_occurrence_checkpoints: usize,
        max_state_reads: usize,
        max_read_calls: usize,
        read_call_gas_limit: u64,
        max_read_call_output_bytes: usize,
    ) -> Self {
        Self {
            max_occurrence_checkpoints,
            max_state_reads,
            max_read_calls,
            read_call_gas_limit,
            max_read_call_output_bytes,
        }
    }
}

#[derive(Debug)]
struct EspaceExecutionIdentity;

/// A retained log state belonging to exactly one finalized execution.
#[derive(Clone)]
pub struct EspaceOccurrenceHandle {
    identity: Arc<EspaceExecutionIdentity>,
    checkpoint_index: usize,
}

impl fmt::Debug for EspaceOccurrenceHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EspaceOccurrenceHandle")
            .field("checkpoint_index", &self.checkpoint_index)
            .finish_non_exhaustive()
    }
}

impl PartialEq for EspaceOccurrenceHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
            && self.checkpoint_index == other.checkpoint_index
    }
}

impl Eq for EspaceOccurrenceHandle {}

/// Initial, committed log and finalized states over one fixed RPC anchor.
pub struct EspaceStateAccess {
    identity: Arc<EspaceExecutionIdentity>,
    source: Arc<ConfluxStateSource>,
    runtime_handle: Handle,
    initial: EspaceStateReader,
    occurrences: Vec<EspaceStateReader>,
    finalized: EspaceStateReader,
    written_accounts: Vec<Address>,
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
        let context = Arc::new(EspaceReadContext {
            machine,
            env: prepared.env.clone(),
            spec: prepared.spec.clone(),
            caller,
            budget: EspaceStateReadBudget::new(limits),
        });
        let written_accounts = collect_written_accounts(&finalized_state);

        Ok(Self {
            identity: Arc::new(EspaceExecutionIdentity),
            source,
            runtime_handle,
            initial: EspaceStateReader::new(initial_state, Arc::clone(&context)),
            occurrences: Vec::new(),
            finalized: EspaceStateReader::new(finalized_state, context),
            written_accounts,
        })
    }

    pub(crate) fn retain_occurrence(
        &mut self,
        snapshot: SavedState,
    ) -> Result<EspaceOccurrenceHandle, EspaceStateAccessError> {
        let mut state = build_conflux_state(Arc::clone(&self.source), self.runtime_handle.clone())
            .map_err(|source| EspaceStateAccessError::Initialization { source })?;
        state.restore(snapshot);
        let checkpoint_index = self.occurrences.len();
        self.occurrences.push(EspaceStateReader::new(
            state,
            Arc::clone(&self.finalized.context),
        ));
        Ok(EspaceOccurrenceHandle {
            identity: Arc::clone(&self.identity),
            checkpoint_index,
        })
    }

    pub const fn initial(&self) -> &EspaceStateReader {
        &self.initial
    }

    pub const fn finalized(&self) -> &EspaceStateReader {
        &self.finalized
    }

    pub(crate) fn written_accounts(&self) -> &[Address] {
        &self.written_accounts
    }

    pub fn at(
        &self,
        occurrence: &EspaceOccurrenceHandle,
    ) -> Result<&EspaceStateReader, EspaceStateReadError> {
        let index = self.checkpoint_index(occurrence)?;
        Ok(&self.occurrences[index])
    }

    /// The previous retained log state (or S0), and this log's state.
    /// The previous point is not necessarily immediately before the LOG opcode.
    pub fn around(
        &self,
        occurrence: &EspaceOccurrenceHandle,
    ) -> Result<EspaceOccurrenceStateReaders<'_>, EspaceStateReadError> {
        let index = self.checkpoint_index(occurrence)?;
        let previous = if index == 0 {
            &self.initial
        } else {
            &self.occurrences[index - 1]
        };
        Ok(EspaceOccurrenceStateReaders {
            previous,
            current: &self.occurrences[index],
        })
    }

    fn checkpoint_index(
        &self,
        occurrence: &EspaceOccurrenceHandle,
    ) -> Result<usize, EspaceStateReadError> {
        if !Arc::ptr_eq(&self.identity, &occurrence.identity) {
            return Err(EspaceStateReadError::ForeignOccurrence);
        }
        // Handles are created only after retaining a reader, and readers are
        // never removed from the finalized state access.
        Ok(occurrence.checkpoint_index)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EspaceOccurrenceStateReaders<'a> {
    previous: &'a EspaceStateReader,
    current: &'a EspaceStateReader,
}

impl<'a> EspaceOccurrenceStateReaders<'a> {
    pub const fn previous(self) -> &'a EspaceStateReader {
        self.previous
    }

    pub const fn current(self) -> &'a EspaceStateReader {
        self.current
    }
}

struct EspaceReadContext {
    machine: Arc<Machine>,
    env: Env,
    spec: Spec,
    caller: Address,
    budget: EspaceStateReadBudget,
}

/// A controlled reader over one fixed eSpace state point.
pub struct EspaceStateReader {
    state: RefCell<State>,
    context: Arc<EspaceReadContext>,
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
    fn new(state: State, context: Arc<EspaceReadContext>) -> Self {
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
        self.context.budget.consume_state_read()?;

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
        self.context.budget.consume_state_read()?;

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
        self.context.budget.consume_read_call()?;

        let mut state = self.state.borrow_mut();
        let outcome = execute_isolated_read_call(
            &mut state,
            &self.context.machine,
            &self.context.env,
            &self.context.spec,
            self.context.caller,
            target,
            calldata.clone(),
            Some(self.context.budget.limits.read_call_gas_limit),
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
        if outcome.output_len() > self.context.budget.limits.max_read_call_output_bytes {
            return Err(EspaceStateReadError::ReadCallOutputLimitExceeded {
                limit: self.context.budget.limits.max_read_call_output_bytes,
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

    #[error("occurrence handle belongs to a different execution")]
    ForeignOccurrence,

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

fn collect_written_accounts(state: &State) -> Vec<Address> {
    let cache = state.cache.read();
    // The active cache overrides the committed cache, including restored entries
    // after a frame rollback. Freeze the set before any analysis read-call.
    let mut accounts: Vec<_> = cache
        .iter()
        .map(|(address, entry)| (address, &entry.entry))
        .chain(
            state
                .committed_cache
                .iter()
                .filter(|(address, _)| !cache.contains_key(address)),
        )
        .filter(|(address, entry)| address.space == Space::Ethereum && entry.is_dirty())
        .map(|(address, _)| address_to_alloy(*address))
        .collect();
    accounts.sort_unstable();
    accounts
}

fn address_to_alloy(address: cfx_types::AddressWithSpace) -> Address {
    Address::from_slice(address.address.as_bytes())
}

#[cfg(test)]
mod tests;

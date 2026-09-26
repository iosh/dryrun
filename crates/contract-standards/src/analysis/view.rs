use std::{borrow::Cow, collections::BTreeSet};

use alloy_primitives::{Address, B256, Bytes, U256};

use super::AnalysisError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadCallOutcome {
    Success(Bytes),
    Reverted(Bytes),
    Halted,
}

/// Reads one immutable state point. Implementations isolate every getter call.
pub trait ContractState {
    fn native_balance(&self, account: Address) -> Result<U256, AnalysisError>;
    fn code(&self, account: Address) -> Result<Bytes, AnalysisError>;
    fn storage(&self, account: Address, slot: B256) -> Result<B256, AnalysisError>;
    fn read_call(&self, target: Address, input: Bytes) -> Result<ReadCallOutcome, AnalysisError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

#[derive(Debug, Clone, Copy)]
pub enum FrameAction<'a> {
    Call {
        kind: CallKind,
        caller: Address,
        target: Address,
        bytecode_address: Address,
        value: U256,
        input: &'a [u8],
    },
    Create {
        caller: Address,
        address: Address,
        value: U256,
        init_code: &'a [u8],
    },
}

/// A borrowed committed call, preserving code identity separately from state ownership.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub id: usize,
    pub parent: Option<usize>,
    pub position: usize,
    pub action: FrameAction<'a>,
    pub code_hash: Option<B256>,
}

impl<'a> Frame<'a> {
    pub const fn id(&self) -> usize {
        self.id
    }
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }
    pub const fn position(&self) -> usize {
        self.position
    }
    pub const fn action(&self) -> &FrameAction<'a> {
        &self.action
    }
}

#[derive(Debug, Clone)]
pub struct LogRef<'a> {
    pub address: Address,
    pub topics: Cow<'a, [B256]>,
    pub data: &'a [u8],
}

#[derive(Debug, Clone)]
pub struct CommittedLog<'a> {
    pub position: usize,
    pub frame_id: usize,
    pub log: LogRef<'a>,
}

/// The preceding retained checkpoint (or initial state), and the current log state.
#[derive(Clone, Copy)]
pub struct StatePair<'a> {
    pub previous: &'a dyn ContractState,
    pub current: &'a dyn ContractState,
}

impl<'a> StatePair<'a> {
    pub const fn previous(self) -> &'a dyn ContractState {
        self.previous
    }
    pub const fn current(self) -> &'a dyn ContractState {
        self.current
    }
}

/// A log and its state, borrowed from the same execution as the preceding checkpoint.
#[derive(Clone)]
pub struct LogCheckpoint<'a> {
    pub position: usize,
    pub frame_id: usize,
    pub log: LogRef<'a>,
    pub states: StatePair<'a>,
}

impl<'a> LogCheckpoint<'a> {
    pub const fn position(&self) -> usize {
        self.position
    }
    pub const fn frame_id(&self) -> usize {
        self.frame_id
    }
    pub const fn log(&self) -> &LogRef<'a> {
        &self.log
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StorageWrite {
    pub position: usize,
    pub frame_id: usize,
    pub address: Address,
    pub slot: Option<B256>,
    pub value: U256,
}

/// Evidence from a single network and Space, borrowed from one execution.
pub trait TokenView {
    /// Call ancestry can extend beyond the assigned facts. Algorithms use this
    /// predicate when interpreting a frame as an operation owned by this rule.
    fn is_in_scope(&self, _position: usize) -> bool {
        true
    }
    fn committed_frames(&self) -> Box<dyn Iterator<Item = Frame<'_>> + '_>;
    fn committed_logs(&self) -> Box<dyn Iterator<Item = CommittedLog<'_>> + '_>;
    /// Selected logs paired with the state readers from the same execution.
    fn log_checkpoints(&self) -> Box<dyn Iterator<Item = LogCheckpoint<'_>> + '_>;
    fn storage_writes(&self) -> Box<dyn Iterator<Item = StorageWrite> + '_>;
    fn initial(&self) -> &dyn ContractState;
    fn finalized(&self) -> &dyn ContractState;
}

/// A contract's facts remain backed by the same execution and immutable state points.
pub struct ContractView<'a> {
    pub execution: &'a dyn TokenView,
    pub address: Address,
    pub positions: &'a BTreeSet<usize>,
}

impl TokenView for ContractView<'_> {
    fn is_in_scope(&self, position: usize) -> bool {
        self.positions.contains(&position) && self.execution.is_in_scope(position)
    }
    fn committed_frames(&self) -> Box<dyn Iterator<Item = Frame<'_>> + '_> {
        // Call ancestry and withdrawal recipients may be outside the token contract.
        self.execution.committed_frames()
    }
    fn committed_logs(&self) -> Box<dyn Iterator<Item = CommittedLog<'_>> + '_> {
        Box::new(
            self.execution
                .committed_logs()
                .filter(|log| log.log.address == self.address && self.is_in_scope(log.position)),
        )
    }
    fn log_checkpoints(&self) -> Box<dyn Iterator<Item = LogCheckpoint<'_>> + '_> {
        Box::new(self.execution.log_checkpoints().filter(|checkpoint| {
            checkpoint.log.address == self.address && self.is_in_scope(checkpoint.position)
        }))
    }
    fn storage_writes(&self) -> Box<dyn Iterator<Item = StorageWrite> + '_> {
        Box::new(
            self.execution
                .storage_writes()
                .filter(|write| write.address == self.address && self.is_in_scope(write.position)),
        )
    }
    fn initial(&self) -> &dyn ContractState {
        self.execution.initial()
    }
    fn finalized(&self) -> &dyn ContractState {
        self.execution.finalized()
    }
}

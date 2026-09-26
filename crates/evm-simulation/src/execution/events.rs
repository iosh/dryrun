use alloy::primitives::{Address, B256, Bytes, Log, U256, keccak256};
use revm::{
    Inspector,
    context::{ContextTr, JournalEntry},
    inspector::JournalExt,
    interpreter::{
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, InstructionResult,
        Interpreter, InterpreterTypes,
        interpreter_types::{InputsTr, Jumps, LegacyBytecode, LoopControl, StackTr},
    },
    state::EvmState,
};
use thiserror::Error;

use crate::EvmSimulationLimits;
use simulation_core::observation::{AnalysisLimitExceeded, LogFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvmExecutionPosition(usize);

impl EvmExecutionPosition {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvmFrameId(usize);

impl EvmFrameId {
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmCallKind {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmFrameAction {
    Call {
        kind: EvmCallKind,
        caller: Address,
        target: Address,
        bytecode_address: Address,
        value: U256,
        input: Bytes,
    },
    Create {
        caller: Address,
        value: U256,
        init_code: Bytes,
        created_address: Option<Address>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmCommittedFrame {
    id: EvmFrameId,
    parent: Option<EvmFrameId>,
    position: EvmExecutionPosition,
    action: EvmFrameAction,
    code_hash: Option<B256>,
}

impl EvmCommittedFrame {
    pub const fn id(&self) -> EvmFrameId {
        self.id
    }

    pub const fn parent(&self) -> Option<EvmFrameId> {
        self.parent
    }

    pub const fn position(&self) -> EvmExecutionPosition {
        self.position
    }

    pub const fn code_hash(&self) -> Option<B256> {
        self.code_hash
    }

    pub const fn action(&self) -> &EvmFrameAction {
        &self.action
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmCommittedLog {
    position: EvmExecutionPosition,
    frame_id: EvmFrameId,
    log: Log,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmCommittedSelfdestruct {
    position: EvmExecutionPosition,
    frame_id: EvmFrameId,
    contract: Address,
    target: Address,
    value: U256,
    destroys_contract: bool,
}

impl EvmCommittedSelfdestruct {
    pub const fn position(&self) -> EvmExecutionPosition {
        self.position
    }

    pub const fn frame_id(&self) -> EvmFrameId {
        self.frame_id
    }

    pub const fn contract(&self) -> Address {
        self.contract
    }

    pub const fn target(&self) -> Address {
        self.target
    }

    pub const fn value(&self) -> U256 {
        self.value
    }

    /// Whether this operation schedules actual contract deletion under the active fork rules.
    pub const fn destroys_contract(&self) -> bool {
        self.destroys_contract
    }
}

impl EvmCommittedLog {
    pub const fn position(&self) -> EvmExecutionPosition {
        self.position
    }

    pub const fn frame_id(&self) -> EvmFrameId {
        self.frame_id
    }

    pub const fn log(&self) -> &Log {
        &self.log
    }
}

/// A committed persistent write, including writes that restore an earlier value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmStorageWrite {
    pub(crate) position: EvmExecutionPosition,
    pub(crate) frame_id: EvmFrameId,
    pub(crate) address: Address,
    pub(crate) slot: U256,
    pub(crate) value: U256,
}

impl EvmStorageWrite {
    pub const fn position(&self) -> EvmExecutionPosition {
        self.position
    }
    pub const fn frame_id(&self) -> EvmFrameId {
        self.frame_id
    }
    pub const fn address(&self) -> Address {
        self.address
    }
    pub const fn slot(&self) -> U256 {
        self.slot
    }
    pub const fn value(&self) -> U256 {
        self.value
    }
}

#[derive(Debug)]
pub(crate) struct EvmExecutionObservation {
    pub(crate) applied_authorization_accounts: Vec<Address>,
    pub(crate) frames: Vec<EvmCommittedFrame>,
    pub(crate) logs: Vec<EvmCommittedLog>,
    pub(crate) selfdestructs: Vec<EvmCommittedSelfdestruct>,
    pub(crate) storage_writes: Vec<EvmStorageWrite>,
    pub(crate) checkpoints: Vec<(usize, EvmState)>,
    pub(crate) limit_exceeded: Option<AnalysisLimitExceeded>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameKind {
    Call,
    Create,
}

#[derive(Debug, Clone)]
struct FrameRollbackPoint {
    frames_len: usize,
    logs_len: usize,
    selfdestructs_len: usize,
    storage_writes_len: usize,
    checkpoints_len: usize,
    limit_exceeded: Option<AnalysisLimitExceeded>,
}

#[derive(Debug)]
struct OpenFrame {
    id: EvmFrameId,
    kind: FrameKind,
    frame_index: Option<usize>,
    rollback: FrameRollbackPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum EvmExecutionObservationError {
    #[error("EVM inspector cannot find the executing account {address}")]
    MissingExecutingAccount { address: Address },

    #[error("EVM inspector observed an unbalanced {callback} frame callback")]
    UnbalancedFrame { callback: &'static str },

    #[error("EVM inspector ended a {actual} frame with a {callback} callback")]
    FrameKindMismatch {
        actual: &'static str,
        callback: &'static str,
    },

    #[error("EVM inspector finished with {open_frames} open frames")]
    OpenFrames { open_frames: usize },
}

#[derive(Debug)]
pub(crate) struct EvmExecutionObserver {
    checkpoint_filters: Vec<LogFilter>,
    limits: EvmSimulationLimits,
    applied_authorization_accounts: Option<Vec<Address>>,
    frames: Vec<EvmCommittedFrame>,
    logs: Vec<EvmCommittedLog>,
    selfdestructs: Vec<EvmCommittedSelfdestruct>,
    storage_writes: Vec<EvmStorageWrite>,
    pending_storage_write: Option<(Address, U256, U256)>,
    pending_selfdestruct: Option<(Address, Address, U256)>,
    checkpoints: Vec<(usize, EvmState)>,
    open_frames: Vec<OpenFrame>,
    next_frame_id: usize,
    next_position: usize,
    limit_exceeded: Option<AnalysisLimitExceeded>,
    observation_error: Option<EvmExecutionObservationError>,
}

impl EvmExecutionObserver {
    pub(crate) fn new(checkpoint_filters: Vec<LogFilter>, limits: EvmSimulationLimits) -> Self {
        Self {
            checkpoint_filters,
            limits,
            applied_authorization_accounts: None,
            frames: Vec::new(),
            logs: Vec::new(),
            selfdestructs: Vec::new(),
            storage_writes: Vec::new(),
            pending_storage_write: None,
            pending_selfdestruct: None,
            checkpoints: Vec::new(),
            open_frames: Vec::new(),
            next_frame_id: 0,
            next_position: 0,
            limit_exceeded: None,
            observation_error: None,
        }
    }

    pub(crate) fn take_observation(
        &mut self,
    ) -> Result<EvmExecutionObservation, EvmExecutionObservationError> {
        let replacement = Self::new(Vec::new(), self.limits);
        std::mem::replace(self, replacement).finish()
    }

    fn finish(self) -> Result<EvmExecutionObservation, EvmExecutionObservationError> {
        if let Some(error) = self.observation_error {
            return Err(error);
        }
        if !self.open_frames.is_empty() {
            return Err(EvmExecutionObservationError::OpenFrames {
                open_frames: self.open_frames.len(),
            });
        }

        Ok(EvmExecutionObservation {
            applied_authorization_accounts: self.applied_authorization_accounts.unwrap_or_default(),
            frames: self.frames,
            logs: self.logs,
            selfdestructs: self.selfdestructs,
            storage_writes: self.storage_writes,
            checkpoints: self.checkpoints,
            limit_exceeded: self.limit_exceeded,
        })
    }

    fn observe_transaction_start<CTX>(&mut self, context: &CTX)
    where
        CTX: ContextTr,
        CTX::Journal: JournalExt,
    {
        if self.applied_authorization_accounts.is_some() {
            return;
        }

        let mut accounts = context
            .journal()
            .journal()
            .iter()
            .filter_map(|entry| match entry {
                JournalEntry::CodeChange { address } => Some(*address),
                _ => None,
            })
            .collect::<Vec<_>>();
        accounts.sort_unstable();
        accounts.dedup();
        if accounts.len() > self.limits.max_observed_facts {
            accounts.truncate(self.limits.max_observed_facts);
            self.limit_exceeded = Some(AnalysisLimitExceeded {
                resource: simulation_core::observation::AnalysisResource::Facts,
                limit: self.limits.max_observed_facts,
            });
        }
        self.applied_authorization_accounts = Some(accounts);
    }

    fn start_frame(&mut self, kind: FrameKind, action: EvmFrameAction) {
        let id = EvmFrameId(self.next_frame_id);
        self.next_frame_id += 1;
        let parent = self.open_frames.last().map(|frame| frame.id);
        let rollback = FrameRollbackPoint {
            frames_len: self.frames.len(),
            logs_len: self.logs.len(),
            selfdestructs_len: self.selfdestructs.len(),
            storage_writes_len: self.storage_writes.len(),
            checkpoints_len: self.checkpoints.len(),
            limit_exceeded: self.limit_exceeded.clone(),
        };
        let position = self.next_position();
        let frame_index = if self.reserve_fact() {
            let index = self.frames.len();
            self.frames.push(EvmCommittedFrame {
                id,
                parent,
                position,
                action,
                code_hash: None,
            });
            Some(index)
        } else {
            None
        };
        self.open_frames.push(OpenFrame {
            id,
            kind,
            frame_index,
            rollback,
        });
    }

    fn end_frame(
        &mut self,
        expected_kind: FrameKind,
        successful: bool,
        created_address: Option<Address>,
    ) {
        let Some(frame) = self.open_frames.pop() else {
            self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                callback: expected_kind.name(),
            });
            return;
        };

        if frame.kind != expected_kind {
            self.record_observation_error(EvmExecutionObservationError::FrameKindMismatch {
                actual: frame.kind.name(),
                callback: expected_kind.name(),
            });
            return;
        }

        if !successful {
            self.storage_writes
                .truncate(frame.rollback.storage_writes_len);
            self.frames.truncate(frame.rollback.frames_len);
            self.logs.truncate(frame.rollback.logs_len);
            self.selfdestructs
                .truncate(frame.rollback.selfdestructs_len);
            self.checkpoints.truncate(frame.rollback.checkpoints_len);
            self.limit_exceeded = frame.rollback.limit_exceeded;
            return;
        }

        if expected_kind == FrameKind::Create {
            let Some(frame_index) = frame.frame_index else {
                return;
            };
            let Some(committed_frame) = self.frames.get_mut(frame_index) else {
                self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                    callback: "create_end",
                });
                return;
            };
            let EvmFrameAction::Create {
                created_address: address,
                ..
            } = &mut committed_frame.action
            else {
                self.record_observation_error(EvmExecutionObservationError::FrameKindMismatch {
                    actual: "call",
                    callback: "create_end",
                });
                return;
            };
            *address = created_address;
        }
    }

    fn observe_log<CTX>(&mut self, context: &CTX, log: Log)
    where
        CTX: ContextTr,
        CTX::Journal: JournalExt,
    {
        let Some(frame_id) = self.open_frames.last().map(|frame| frame.id) else {
            self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                callback: "log",
            });
            return;
        };

        let position = self.next_position();
        if !self.reserve_fact() {
            return;
        }
        let log_index = self.logs.len();
        let checkpoint_candidate = log.data.topics().first().is_some_and(|topic| {
            self.checkpoint_filters
                .iter()
                .any(|filter| filter.matches(log.address, *topic))
        });
        self.logs.push(EvmCommittedLog {
            position,
            frame_id,
            log,
        });

        if checkpoint_candidate {
            self.checkpoints
                .push((log_index, context.journal().evm_state().clone()));
        }
    }

    fn observe_selfdestruct(
        &mut self,
        contract: Address,
        target: Address,
        value: U256,
        destroys_contract: bool,
    ) {
        let Some(frame_id) = self.open_frames.last().map(|frame| frame.id) else {
            self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                callback: "selfdestruct",
            });
            return;
        };

        let position = self.next_position();
        if !self.reserve_fact() {
            return;
        }
        self.selfdestructs.push(EvmCommittedSelfdestruct {
            position,
            frame_id,
            contract,
            target,
            value,
            destroys_contract,
        });
    }

    fn reserve_fact(&mut self) -> bool {
        let facts = self.frames.len()
            + self.logs.len()
            + self.selfdestructs.len()
            + self.storage_writes.len()
            + self
                .applied_authorization_accounts
                .as_ref()
                .map_or(0, Vec::len);
        if facts >= self.limits.max_observed_facts {
            self.limit_exceeded.get_or_insert(AnalysisLimitExceeded {
                resource: simulation_core::observation::AnalysisResource::Facts,
                limit: self.limits.max_observed_facts,
            });
        }
        self.limit_exceeded.is_none()
    }

    fn next_position(&mut self) -> EvmExecutionPosition {
        let position = EvmExecutionPosition(self.next_position);
        self.next_position += 1;
        position
    }

    fn record_observation_error(&mut self, error: EvmExecutionObservationError) {
        if self.observation_error.is_none() {
            self.observation_error = Some(error);
        }
    }
}

impl FrameKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Create => "create",
        }
    }
}

impl From<CallScheme> for EvmCallKind {
    fn from(value: CallScheme) -> Self {
        match value {
            CallScheme::Call => Self::Call,
            CallScheme::CallCode => Self::CallCode,
            CallScheme::DelegateCall => Self::DelegateCall,
            CallScheme::StaticCall => Self::StaticCall,
        }
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for EvmExecutionObserver
where
    CTX: ContextTr,
    CTX::Journal: JournalExt,
    INTR: InterpreterTypes,
{
    fn initialize_interp(&mut self, interpreter: &mut Interpreter<INTR>, context: &mut CTX) {
        if let Some(frame) = self.open_frames.last() {
            if let Some(record) = frame
                .frame_index
                .and_then(|index| self.frames.get_mut(index))
            {
                record.code_hash = Some(keccak256(interpreter.bytecode.bytecode_slice()));
                if let EvmFrameAction::Call {
                    bytecode_address, ..
                } = &mut record.action
                {
                    if let Some(actual) = interpreter.input.bytecode_address() {
                        *bytecode_address = context
                            .journal()
                            .evm_state()
                            .get(actual)
                            .and_then(|account| account.info.code.as_ref())
                            .and_then(|code| code.eip7702_address())
                            .unwrap_or(*actual);
                    }
                }
            }
        }
    }

    fn step(&mut self, interpreter: &mut Interpreter<INTR>, context: &mut CTX) {
        self.pending_storage_write = None;
        self.pending_selfdestruct = None;
        if interpreter.bytecode.opcode() == revm::bytecode::opcode::SSTORE {
            if let [.., value, slot] = interpreter.stack.data() {
                self.pending_storage_write =
                    Some((interpreter.input.target_address(), *slot, *value));
            }
        } else if interpreter.bytecode.opcode() == revm::bytecode::opcode::SELFDESTRUCT {
            if let Some(target) = interpreter.stack.data().last() {
                let contract = interpreter.input.target_address();
                if let Some(account) = context.journal().evm_state().get(&contract) {
                    self.pending_selfdestruct = Some((
                        contract,
                        Address::from_word(B256::from(*target)),
                        account.info.balance,
                    ));
                } else {
                    self.record_observation_error(
                        EvmExecutionObservationError::MissingExecutingAccount { address: contract },
                    );
                }
            }
        }
    }

    fn step_end(&mut self, interpreter: &mut Interpreter<INTR>, context: &mut CTX) {
        if let Some((contract, target, value)) = self.pending_selfdestruct.take() {
            // REVM's generic callback reads the last journal entry. A Cancun self-call
            // creates no entry, so that callback can describe an earlier operation.
            if interpreter.bytecode.instruction_result() == Some(InstructionResult::SelfDestruct) {
                if let Some(account) = context.journal().evm_state().get(&contract) {
                    self.observe_selfdestruct(contract, target, value, account.is_selfdestructed());
                } else {
                    self.record_observation_error(
                        EvmExecutionObservationError::MissingExecutingAccount { address: contract },
                    );
                }
            }
        }
        let Some((address, slot, value)) = self.pending_storage_write.take() else {
            return;
        };
        if !interpreter.bytecode.is_not_end() {
            return;
        }
        let Some(frame_id) = self.open_frames.last().map(|frame| frame.id) else {
            self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                callback: "storage write",
            });
            return;
        };
        let position = self.next_position();
        if self.reserve_fact() {
            self.storage_writes.push(EvmStorageWrite {
                position,
                frame_id,
                address,
                slot,
                value,
            });
        }
    }

    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.observe_transaction_start(context);
        self.start_frame(
            FrameKind::Call,
            EvmFrameAction::Call {
                kind: inputs.scheme.into(),
                caller: inputs.caller,
                target: inputs.target_address,
                bytecode_address: inputs.bytecode_address,
                value: inputs.call_value(),
                input: inputs.input.bytes(context),
            },
        );
        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.end_frame(FrameKind::Call, outcome.instruction_result().is_ok(), None);
    }

    fn create(&mut self, context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.observe_transaction_start(context);
        self.start_frame(
            FrameKind::Create,
            EvmFrameAction::Create {
                caller: inputs.caller(),
                value: inputs.value(),
                init_code: inputs.init_code().clone(),
                created_address: None,
            },
        );
        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.end_frame(
            FrameKind::Create,
            outcome.instruction_result().is_ok(),
            outcome.address,
        );
    }

    fn log(&mut self, context: &mut CTX, log: Log) {
        self.observe_log(context, log);
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, Bytes, U256, keccak256};
    use revm::{
        Context, InspectEvm, MainBuilder, MainContext,
        context::TxEnv,
        database::{BENCH_CALLER, BENCH_TARGET, BenchmarkDB, InMemoryDB},
        primitives::TxKind,
        state::{AccountInfo, Bytecode, bytecode::opcode},
    };

    use simulation_core::observation::LogFilter;

    use super::{EvmExecutionObservation, EvmExecutionObserver};

    #[test]
    fn committed_candidate_retains_its_state_checkpoint() {
        let mut code = vec![opcode::PUSH1, 100, opcode::PUSH0, opcode::SSTORE];
        push_log(&mut code, B256::repeat_byte(0xff));
        push_log(&mut code, keccak256("Approval(address,address,uint256)"));
        code.push(opcode::STOP);

        let (result_logs, observation) = execute(code);

        assert_eq!(result_logs, 2);
        assert_eq!(observation.logs.len(), 2);
        assert_eq!(observation.checkpoints.len(), 1);
        let stored =
            observation.checkpoints[0].1[&BENCH_TARGET].storage[&U256::ZERO].present_value();
        assert_eq!(stored, U256::from(100));
    }

    #[test]
    fn parent_revert_discards_a_successful_child_occurrence() {
        let child = Address::repeat_byte(0x11);
        let mut child_code = vec![opcode::PUSH1, 100, opcode::PUSH0, opcode::SSTORE];
        push_log(
            &mut child_code,
            keccak256("Approval(address,address,uint256)"),
        );
        child_code.push(opcode::STOP);

        let mut parent_code = vec![
            opcode::PUSH0,
            opcode::PUSH0,
            opcode::PUSH0,
            opcode::PUSH0,
            opcode::PUSH0,
            opcode::PUSH20,
        ];
        parent_code.extend_from_slice(child.as_slice());
        parent_code.extend([
            opcode::PUSH2,
            0xff,
            0xff,
            opcode::CALL,
            opcode::POP,
            opcode::PUSH0,
            opcode::PUSH0,
            opcode::REVERT,
        ]);

        let mut database = InMemoryDB::default();
        database.insert_account_info(
            BENCH_TARGET,
            AccountInfo::default().with_code(Bytecode::new_raw(Bytes::from(parent_code))),
        );
        database.insert_account_info(
            child,
            AccountInfo::default().with_code(Bytecode::new_raw(Bytes::from(child_code))),
        );
        database.insert_account_info(
            BENCH_CALLER,
            AccountInfo {
                balance: U256::MAX,
                ..Default::default()
            },
        );

        let (result_logs, observation) = execute_database(database);

        assert_eq!(result_logs, 0);
        assert!(observation.frames.is_empty());
        assert!(observation.logs.is_empty());
        assert!(observation.checkpoints.is_empty());
    }

    fn execute(code: Vec<u8>) -> (usize, EvmExecutionObservation) {
        let database = BenchmarkDB::new_bytecode(Bytecode::new_raw(Bytes::from(code)));
        let context = Context::mainnet().with_db(database);
        let mut evm = context.build_mainnet_with_inspector(EvmExecutionObserver::new(
            approval_checkpoint_filters(),
            crate::EvmSimulationLimits::default(),
        ));
        execute_evm(&mut evm)
    }

    fn execute_database(database: InMemoryDB) -> (usize, EvmExecutionObservation) {
        let context = Context::mainnet().with_db(database);
        let mut evm = context.build_mainnet_with_inspector(EvmExecutionObserver::new(
            approval_checkpoint_filters(),
            crate::EvmSimulationLimits::default(),
        ));
        execute_evm(&mut evm)
    }

    fn approval_checkpoint_filters() -> Vec<LogFilter> {
        vec![LogFilter {
            address: None,
            topic0: keccak256("Approval(address,address,uint256)"),
        }]
    }

    fn execute_evm<DB>(
        evm: &mut revm::MainnetEvm<
            revm::Context<revm::context::BlockEnv, TxEnv, revm::context::CfgEnv, DB>,
            EvmExecutionObserver,
        >,
    ) -> (usize, EvmExecutionObservation)
    where
        DB: revm::Database,
        DB::Error: core::fmt::Debug,
    {
        let result = evm
            .inspect_tx(
                TxEnv::builder()
                    .caller(BENCH_CALLER)
                    .kind(TxKind::Call(BENCH_TARGET))
                    .gas_limit(5_000_000)
                    .build()
                    .expect("test transaction should be valid"),
            )
            .expect("test execution should complete");
        let observation = evm
            .inspector
            .take_observation()
            .expect("observer should finalize");
        (result.result.logs().len(), observation)
    }

    fn push_log(code: &mut Vec<u8>, topic: B256) {
        code.push(opcode::PUSH32);
        code.extend_from_slice(topic.as_slice());
        code.extend([opcode::PUSH0, opcode::PUSH0, opcode::LOG1]);
    }
}

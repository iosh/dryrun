use std::sync::LazyLock;

use alloy::primitives::{Address, B256, Bytes, Log, U256, keccak256};
use contract_standards::is_supported_event_topic;
use revm::{
    Inspector,
    context::{ContextTr, JournalEntry},
    inspector::JournalExt,
    interpreter::{
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, InterpreterTypes,
    },
    state::EvmState,
};
use thiserror::Error;

const MAX_OCCURRENCE_CHECKPOINTS: usize = 256;
const MAX_RETAINED_STATE_UNITS: usize = 1_000_000;

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

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
pub enum EvmCommittedFrameKind {
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
    kind: EvmCommittedFrameKind,
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

    pub const fn kind(&self) -> &EvmCommittedFrameKind {
        &self.kind
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

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EvmOccurrenceEvidenceError {
    #[error("semantic occurrence checkpoint limit {limit} exceeded")]
    CheckpointLimitExceeded { limit: usize },

    #[error("semantic occurrence retained state limit {limit} exceeded")]
    RetainedStateLimitExceeded { limit: usize },
}

#[derive(Debug)]
pub(crate) struct EvmExecutionObservation {
    pub(crate) applied_authorization_accounts: Vec<Address>,
    pub(crate) frames: Vec<EvmCommittedFrame>,
    pub(crate) logs: Vec<EvmCommittedLog>,
    pub(crate) selfdestructs: Vec<EvmCommittedSelfdestruct>,
    pub(crate) semantic_logs: Vec<ObservedSemanticLog>,
    pub(crate) checkpoints: Vec<EvmState>,
    pub(crate) evidence_error: Option<EvmOccurrenceEvidenceError>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ObservedSemanticLog {
    pub(crate) log_index: usize,
    pub(crate) checkpoint_index: Option<usize>,
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
    semantic_logs_len: usize,
    checkpoints_len: usize,
    retained_state_units: usize,
    evidence_error: Option<EvmOccurrenceEvidenceError>,
}

#[derive(Debug)]
struct OpenFrame {
    id: EvmFrameId,
    kind: FrameKind,
    frame_index: usize,
    rollback: FrameRollbackPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum EvmExecutionObservationError {
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

#[derive(Debug, Default)]
pub(crate) struct EvmExecutionObserver {
    wrapped_native_token: Option<Address>,
    applied_authorization_accounts: Option<Vec<Address>>,
    frames: Vec<EvmCommittedFrame>,
    logs: Vec<EvmCommittedLog>,
    selfdestructs: Vec<EvmCommittedSelfdestruct>,
    semantic_logs: Vec<ObservedSemanticLog>,
    checkpoints: Vec<EvmState>,
    open_frames: Vec<OpenFrame>,
    next_frame_id: usize,
    next_position: usize,
    retained_state_units: usize,
    evidence_error: Option<EvmOccurrenceEvidenceError>,
    observation_error: Option<EvmExecutionObservationError>,
}

impl EvmExecutionObserver {
    pub(crate) fn new(wrapped_native_token: Option<Address>) -> Self {
        Self {
            wrapped_native_token,
            ..Self::default()
        }
    }

    pub(crate) fn take_observation(
        &mut self,
    ) -> Result<EvmExecutionObservation, EvmExecutionObservationError> {
        std::mem::take(self).finish()
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
            semantic_logs: self.semantic_logs,
            checkpoints: self.checkpoints,
            evidence_error: self.evidence_error,
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
        self.applied_authorization_accounts = Some(accounts);
    }

    fn start_frame(&mut self, kind: FrameKind, frame_kind: EvmCommittedFrameKind) {
        let id = EvmFrameId(self.next_frame_id);
        self.next_frame_id += 1;
        let parent = self.open_frames.last().map(|frame| frame.id);
        let rollback = FrameRollbackPoint {
            frames_len: self.frames.len(),
            logs_len: self.logs.len(),
            selfdestructs_len: self.selfdestructs.len(),
            semantic_logs_len: self.semantic_logs.len(),
            checkpoints_len: self.checkpoints.len(),
            retained_state_units: self.retained_state_units,
            evidence_error: self.evidence_error.clone(),
        };
        let frame_index = self.frames.len();
        let position = self.next_position();
        self.frames.push(EvmCommittedFrame {
            id,
            parent,
            position,
            kind: frame_kind,
        });
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
            self.frames.truncate(frame.rollback.frames_len);
            self.logs.truncate(frame.rollback.logs_len);
            self.selfdestructs
                .truncate(frame.rollback.selfdestructs_len);
            self.semantic_logs
                .truncate(frame.rollback.semantic_logs_len);
            self.checkpoints.truncate(frame.rollback.checkpoints_len);
            self.retained_state_units = frame.rollback.retained_state_units;
            self.evidence_error = frame.rollback.evidence_error;
            return;
        }

        if expected_kind == FrameKind::Create {
            let Some(committed_frame) = self.frames.get_mut(frame.frame_index) else {
                self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                    callback: "create_end",
                });
                return;
            };
            let EvmCommittedFrameKind::Create {
                created_address: address,
                ..
            } = &mut committed_frame.kind
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
        let log_index = self.logs.len();
        let checkpoint_candidate = log
            .data
            .topics()
            .first()
            .is_some_and(|topic| self.is_checkpoint_candidate(log.address, topic));
        self.logs.push(EvmCommittedLog {
            position,
            frame_id,
            log,
        });

        if !checkpoint_candidate {
            return;
        }

        let checkpoint_index = self.capture_checkpoint(context.journal().evm_state());
        self.semantic_logs.push(ObservedSemanticLog {
            log_index,
            checkpoint_index,
        });
    }

    fn observe_selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        let Some(frame_id) = self.open_frames.last().map(|frame| frame.id) else {
            self.record_observation_error(EvmExecutionObservationError::UnbalancedFrame {
                callback: "selfdestruct",
            });
            return;
        };

        let position = self.next_position();
        self.selfdestructs.push(EvmCommittedSelfdestruct {
            position,
            frame_id,
            contract,
            target,
            value,
        });
    }

    fn is_checkpoint_candidate(&self, address: Address, topic: &B256) -> bool {
        is_supported_event_topic(topic)
            || (self.wrapped_native_token == Some(address)
                && (topic == &*DEPOSIT_TOPIC0 || topic == &*WITHDRAWAL_TOPIC0))
    }

    fn capture_checkpoint(&mut self, state: &EvmState) -> Option<usize> {
        if self.evidence_error.is_some() {
            return None;
        }
        if self.checkpoints.len() >= MAX_OCCURRENCE_CHECKPOINTS {
            self.evidence_error = Some(EvmOccurrenceEvidenceError::CheckpointLimitExceeded {
                limit: MAX_OCCURRENCE_CHECKPOINTS,
            });
            return None;
        }

        let state_units = retained_state_units(state);
        let Some(retained_state_units) = self.retained_state_units.checked_add(state_units) else {
            self.evidence_error = Some(EvmOccurrenceEvidenceError::RetainedStateLimitExceeded {
                limit: MAX_RETAINED_STATE_UNITS,
            });
            return None;
        };
        if retained_state_units > MAX_RETAINED_STATE_UNITS {
            self.evidence_error = Some(EvmOccurrenceEvidenceError::RetainedStateLimitExceeded {
                limit: MAX_RETAINED_STATE_UNITS,
            });
            return None;
        }

        let checkpoint_index = self.checkpoints.len();
        self.checkpoints.push(state.clone());
        self.retained_state_units = retained_state_units;
        Some(checkpoint_index)
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
    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.observe_transaction_start(context);
        self.start_frame(
            FrameKind::Call,
            EvmCommittedFrameKind::Call {
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
            EvmCommittedFrameKind::Create {
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

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        self.observe_selfdestruct(contract, target, value);
    }
}

fn retained_state_units(state: &EvmState) -> usize {
    state.values().fold(state.len(), |units, account| {
        units.saturating_add(account.storage.len())
    })
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

    use super::{
        EvmExecutionObservation, EvmExecutionObserver, EvmOccurrenceEvidenceError,
        MAX_OCCURRENCE_CHECKPOINTS,
    };

    #[test]
    fn committed_candidate_retains_its_state_checkpoint() {
        let mut code = vec![opcode::PUSH1, 100, opcode::PUSH0, opcode::SSTORE];
        push_log(&mut code, B256::repeat_byte(0xff));
        push_log(&mut code, keccak256("Approval(address,address,uint256)"));
        code.push(opcode::STOP);

        let (result_logs, observation) = execute(code);

        assert_eq!(result_logs, 2);
        assert_eq!(observation.logs.len(), 2);
        assert_eq!(observation.semantic_logs.len(), 1);
        assert_eq!(observation.checkpoints.len(), 1);
        let stored = observation.checkpoints[0][&BENCH_TARGET].storage[&U256::ZERO].present_value();
        assert_eq!(stored, U256::from(100));
    }

    #[test]
    fn parent_revert_discards_a_successful_child_occurrence() {
        let child = Address::with_last_byte(1);
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
        assert!(observation.semantic_logs.is_empty());
        assert!(observation.checkpoints.is_empty());
    }

    #[test]
    fn committed_checkpoint_limit_makes_occurrence_evidence_unavailable() {
        let mut code = Vec::new();
        for _ in 0..=MAX_OCCURRENCE_CHECKPOINTS {
            push_log(&mut code, keccak256("Approval(address,address,uint256)"));
        }
        code.push(opcode::STOP);

        let (result_logs, observation) = execute(code);

        assert_eq!(result_logs, MAX_OCCURRENCE_CHECKPOINTS + 1);
        assert_eq!(
            observation.semantic_logs.len(),
            MAX_OCCURRENCE_CHECKPOINTS + 1
        );
        assert_eq!(observation.checkpoints.len(), MAX_OCCURRENCE_CHECKPOINTS);
        assert!(matches!(
            observation.evidence_error,
            Some(EvmOccurrenceEvidenceError::CheckpointLimitExceeded {
                limit: MAX_OCCURRENCE_CHECKPOINTS
            })
        ));
    }

    fn execute(code: Vec<u8>) -> (usize, EvmExecutionObservation) {
        let database = BenchmarkDB::new_bytecode(Bytecode::new_raw(Bytes::from(code)));
        let context = Context::mainnet().with_db(database);
        let mut evm = context.build_mainnet_with_inspector(EvmExecutionObserver::new(None));
        execute_evm(&mut evm)
    }

    fn execute_database(database: InMemoryDB) -> (usize, EvmExecutionObservation) {
        let context = Context::mainnet().with_db(database);
        let mut evm = context.build_mainnet_with_inspector(EvmExecutionObserver::new(None));
        execute_evm(&mut evm)
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

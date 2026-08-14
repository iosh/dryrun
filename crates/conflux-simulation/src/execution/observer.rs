use cfx_executor::{
    executive_observer::{
        AddressPocket, AsTracer, CallTracer, CheckpointTracer, DrainTrace, InternalTransferTracer,
        OpcodeTracer, SetAuthTracer, StorageTracer, TracerTrait,
    },
    stack::{FrameResult, FrameReturn},
};
use cfx_types::{Address, H256, Space, U256};
use cfx_vm_types::{ActionParams, ActionValue, CallType};
use typemap::ShareDebugMap;

// transferFrom(address,address,uint256) is a selector plus three ABI words.
const CALLDATA_PREFIX_LIMIT: usize = 100;

pub(crate) type TracePosition = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FrameId(usize);

#[derive(Debug, PartialEq)]
pub(crate) struct TraceFrame {
    pub(crate) parent_id: Option<FrameId>,
    pub(crate) space: Space,
    pub(crate) action: FrameAction,
}

#[derive(Debug, PartialEq)]
pub(crate) enum FrameAction {
    Call {
        call_type: CallType,
        caller: Address,
        target: Address,
        code_address: Address,
        transferred_value: U256,
        calldata_len: usize,
        calldata_prefix: Vec<u8>,
    },
    Create {
        creator: Address,
        created_address: Address,
        value: U256,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) enum TraceEvent {
    FrameStart {
        position: TracePosition,
        frame_id: FrameId,
    },
    Log {
        position: TracePosition,
        frame_id: FrameId,
        address: Address,
        topics: Vec<H256>,
        data: Vec<u8>,
    },
    InternalTransfer {
        position: TracePosition,
        frame_id: Option<FrameId>,
        space: Space,
        from: AddressPocket,
        to: AddressPocket,
        value: U256,
    },
}

impl TraceEvent {
    pub(crate) const fn position(&self) -> TracePosition {
        match self {
            Self::FrameStart { position, .. }
            | Self::Log { position, .. }
            | Self::InternalTransfer { position, .. } => *position,
        }
    }

    pub(crate) const fn frame_id(&self) -> Option<FrameId> {
        match self {
            Self::FrameStart { frame_id, .. } | Self::Log { frame_id, .. } => Some(*frame_id),
            Self::InternalTransfer { frame_id, .. } => *frame_id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommittedExecutionTrace {
    frames_by_id: Vec<Option<TraceFrame>>,
    events: Vec<TraceEvent>,
}

impl CommittedExecutionTrace {
    pub(crate) fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub(crate) fn frame(&self, frame_id: FrameId) -> &TraceFrame {
        self.frames_by_id[frame_id.0]
            .as_ref()
            .expect("committed trace event references a committed frame")
    }

    pub(crate) fn internal_transfers_in_scope(
        &self,
        frame_id: Option<FrameId>,
    ) -> impl Iterator<Item = &TraceEvent> {
        self.events.iter().filter(move |event| {
            matches!(
                event,
                TraceEvent::InternalTransfer {
                    frame_id: event_frame_id,
                    ..
                } if *event_frame_id == frame_id
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameType {
    Call,
    Create,
}

#[derive(Debug, Clone, Copy)]
struct JournalMark {
    frame_count: usize,
    event_count: usize,
}

#[derive(Debug)]
struct ActiveFrame {
    id: FrameId,
    frame_type: FrameType,
    rollback_mark: JournalMark,
}

#[derive(Debug)]
struct ExecutionTraceJournal {
    frames_by_id: Vec<Option<TraceFrame>>,
    events: Vec<TraceEvent>,
    active_frames: Vec<ActiveFrame>,
    checkpoints: Vec<JournalMark>,
    next_event_position: TracePosition,
    invalid_sequence: bool,
    transaction_space: Space,
}

impl ExecutionTraceJournal {
    fn new(transaction_space: Space) -> Self {
        Self {
            frames_by_id: Vec::new(),
            events: Vec::new(),
            active_frames: Vec::new(),
            checkpoints: Vec::new(),
            next_event_position: 0,
            invalid_sequence: false,
            transaction_space,
        }
    }

    fn allocate_event_position(&mut self) -> TracePosition {
        let position = self.next_event_position;
        if let Some(next) = self.next_event_position.checked_add(1) {
            self.next_event_position = next;
        } else {
            self.invalid_sequence = true;
        }
        position
    }

    fn enter_call_frame(&mut self, params: &ActionParams) {
        let calldata = params.data.as_deref().unwrap_or_default();
        let frame = TraceFrame {
            parent_id: self.active_frames.last().map(|frame| frame.id),
            space: params.space,
            action: FrameAction::Call {
                call_type: params.call_type,
                caller: params.sender,
                target: params.address,
                code_address: params.code_address,
                transferred_value: actual_transfer_value(&params.value),
                calldata_len: calldata.len(),
                calldata_prefix: calldata
                    .iter()
                    .copied()
                    .take(calldata_bytes_to_capture(
                        params.space,
                        params.address,
                        params.code_address,
                    ))
                    .collect(),
            },
        };
        self.enter_frame(frame, FrameType::Call);
    }

    fn enter_create_frame(&mut self, params: &ActionParams) {
        let frame = TraceFrame {
            parent_id: self.active_frames.last().map(|frame| frame.id),
            space: params.space,
            action: FrameAction::Create {
                creator: params.sender,
                created_address: params.address,
                value: actual_transfer_value(&params.value),
            },
        };
        self.enter_frame(frame, FrameType::Create);
    }

    fn enter_frame(&mut self, frame: TraceFrame, frame_type: FrameType) {
        let id = FrameId(self.frames_by_id.len());
        let rollback_mark = self.mark();
        self.frames_by_id.push(Some(frame));
        let position = self.allocate_event_position();
        self.events.push(TraceEvent::FrameStart {
            position,
            frame_id: id,
        });
        self.active_frames.push(ActiveFrame {
            id,
            frame_type,
            rollback_mark,
        });
    }

    fn exit_call_frame(&mut self, result: &FrameResult) {
        self.exit_frame(FrameType::Call, frame_succeeded(result));
    }

    fn exit_create_frame(&mut self, result: &FrameResult) {
        self.exit_frame(FrameType::Create, frame_succeeded(result));
    }

    fn exit_frame(&mut self, expected_type: FrameType, success: bool) {
        let Some(frame) = self.active_frames.pop() else {
            self.invalid_sequence = true;
            return;
        };

        if frame.frame_type != expected_type {
            self.invalid_sequence = true;
            return;
        }

        if !success {
            self.rollback_to(frame.rollback_mark);
        }
    }

    fn record_log(&mut self, address: Address, topics: &[H256], data: &[u8]) {
        let Some(frame_id) = self.active_frames.last().map(|frame| frame.id) else {
            self.invalid_sequence = true;
            return;
        };

        let position = self.allocate_event_position();
        self.events.push(TraceEvent::Log {
            position,
            frame_id,
            address,
            topics: topics.to_vec(),
            data: data.to_vec(),
        });
    }

    fn record_internal_transfer(&mut self, from: AddressPocket, to: AddressPocket, value: U256) {
        let frame_id = self.active_frames.last().map(|frame| frame.id);
        let space = frame_id.map_or(self.transaction_space, |id| {
            self.frames_by_id[id.0]
                .as_ref()
                .expect("active frame has trace metadata")
                .space
        });
        let position = self.allocate_event_position();
        self.events.push(TraceEvent::InternalTransfer {
            position,
            frame_id,
            space,
            from,
            to,
            value,
        });
    }

    fn checkpoint(&mut self) {
        self.checkpoints.push(self.mark());
    }

    fn commit_checkpoint(&mut self) {
        if !self.active_frames.is_empty() || self.checkpoints.pop().is_none() {
            self.invalid_sequence = true;
        }
    }

    fn revert_checkpoint(&mut self) {
        if !self.active_frames.is_empty() {
            self.invalid_sequence = true;
        }
        let Some(checkpoint) = self.checkpoints.pop() else {
            self.invalid_sequence = true;
            return;
        };
        self.rollback_to(checkpoint);
    }

    fn mark(&self) -> JournalMark {
        JournalMark {
            frame_count: self.frames_by_id.len(),
            event_count: self.events.len(),
        }
    }

    fn rollback_to(&mut self, mark: JournalMark) {
        self.events.truncate(mark.event_count);
        for frame in &mut self.frames_by_id[mark.frame_count..] {
            *frame = None;
        }
    }

    fn into_committed_trace(mut self) -> Option<CommittedExecutionTrace> {
        if !self.active_frames.is_empty() || !self.checkpoints.is_empty() {
            self.invalid_sequence = true;
        }
        (!self.invalid_sequence).then_some(CommittedExecutionTrace {
            frames_by_id: self.frames_by_id,
            events: self.events,
        })
    }
}

fn frame_succeeded(result: &FrameResult) -> bool {
    matches!(
        result,
        Ok(FrameReturn {
            apply_state: true,
            ..
        })
    )
}

fn actual_transfer_value(value: &ActionValue) -> U256 {
    match value {
        ActionValue::Transfer(value) => *value,
        ActionValue::Apparent(_) => U256::zero(),
    }
}

fn calldata_bytes_to_capture(space: Space, target: Address, code_address: Address) -> usize {
    let sponsor_contract =
        cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
    let admin_contract =
        cfx_parameters::internal_contract_addresses::ADMIN_CONTROL_CONTRACT_ADDRESS;
    if space == Space::Native
        && (target == sponsor_contract
            || code_address == sponsor_contract
            || target == admin_contract
            || code_address == admin_contract)
    {
        usize::MAX
    } else {
        CALLDATA_PREFIX_LIMIT
    }
}

#[derive(Debug)]
pub(crate) struct ExecutionTraceObserver {
    journal: ExecutionTraceJournal,
}

impl ExecutionTraceObserver {
    pub(crate) fn new(transaction_space: Space) -> Self {
        Self {
            journal: ExecutionTraceJournal::new(transaction_space),
        }
    }
}

pub(crate) struct ExecutionTraceKey;

impl typemap::Key for ExecutionTraceKey {
    type Value = CommittedExecutionTrace;
}

impl AsTracer for ExecutionTraceObserver {
    fn as_tracer<'a>(&'a mut self) -> Box<dyn 'a + TracerTrait> {
        Box::new(self)
    }
}

impl DrainTrace for ExecutionTraceObserver {
    fn drain_trace(self, map: &mut ShareDebugMap) {
        if let Some(trace) = self.journal.into_committed_trace() {
            map.insert::<ExecutionTraceKey>(trace);
        }
    }
}

impl CallTracer for ExecutionTraceObserver {
    fn record_call(&mut self, params: &ActionParams) {
        self.journal.enter_call_frame(params);
    }

    fn record_call_result(&mut self, result: &FrameResult) {
        self.journal.exit_call_frame(result);
    }

    fn record_create(&mut self, params: &ActionParams) {
        self.journal.enter_create_frame(params);
    }

    fn record_create_result(&mut self, result: &FrameResult) {
        self.journal.exit_create_frame(result);
    }
}

impl CheckpointTracer for ExecutionTraceObserver {
    fn trace_checkpoint(&mut self) {
        self.journal.checkpoint();
    }

    fn trace_checkpoint_discard(&mut self) {
        self.journal.commit_checkpoint();
    }

    fn trace_checkpoint_revert(&mut self) {
        self.journal.revert_checkpoint();
    }
}

impl InternalTransferTracer for ExecutionTraceObserver {
    fn trace_internal_transfer(&mut self, from: AddressPocket, to: AddressPocket, value: U256) {
        self.journal.record_internal_transfer(from, to, value);
    }
}

impl OpcodeTracer for ExecutionTraceObserver {
    fn log(&mut self, address: &Address, topics: &Vec<H256>, data: &[u8]) {
        self.journal.record_log(*address, topics, data);
    }
}

impl SetAuthTracer for ExecutionTraceObserver {}
impl StorageTracer for ExecutionTraceObserver {}

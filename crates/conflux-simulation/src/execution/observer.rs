use cfx_executor::{
    executive_observer::{
        AddressPocket, AsTracer, CallTracer, CheckpointTracer, DrainTrace, InternalTransferTracer,
        OpcodeTracer, SetAuthTracer, StorageTracer, TracerTrait,
    },
    stack::{FrameResult, FrameReturn},
    state::{SavedState, State},
};
use cfx_parity_trace_types::{SetAuth, SetAuthOutcome};
use cfx_types::{Address, AddressWithSpace, H256, Space, U256};
use cfx_vm_types::{ActionParams, ActionValue, CallType};
use simulation_core::observation::{
    AnalysisLimitExceeded, AnalysisLimits, AnalysisResource, LogFilter,
};
use std::collections::BTreeMap;
use typemap::ShareDebugMap;

pub(crate) type TracePosition = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FrameId(usize);

impl FrameId {
    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TraceFrame {
    pub(crate) parent_id: Option<FrameId>,
    pub(crate) space: Space,
    /// Hash supplied with the executable code by the VM.
    pub(crate) code_hash: H256,
    pub(crate) action: FrameAction,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FrameAction {
    Call {
        call_type: CallType,
        caller: Address,
        target: Address,
        /// Code lookup account; the callback does not expose an eSpace delegation target.
        code_address: Address,
        transferred_value: U256,
        calldata: Vec<u8>,
    },
    Create {
        creator: Address,
        created_address: Address,
        actual_created_address: Option<Address>,
        value: U256,
        init_code: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TraceEvent {
    ContractRemoved {
        position: TracePosition,
        address: AddressWithSpace,
    },
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
    StorageWrite {
        position: TracePosition,
        frame_id: FrameId,
        address: AddressWithSpace,
        key: Vec<u8>,
        value: U256,
    },
}

impl TraceEvent {
    pub(crate) const fn position(&self) -> TracePosition {
        match self {
            Self::ContractRemoved { position, .. }
            | Self::FrameStart { position, .. }
            | Self::Log { position, .. }
            | Self::InternalTransfer { position, .. }
            | Self::StorageWrite { position, .. } => *position,
        }
    }

    #[cfg(test)]
    pub(crate) const fn frame_id(&self) -> Option<FrameId> {
        match self {
            Self::FrameStart { frame_id, .. } | Self::Log { frame_id, .. } => Some(*frame_id),
            Self::InternalTransfer { frame_id, .. } => *frame_id,
            Self::StorageWrite { frame_id, .. } => Some(*frame_id),
            Self::ContractRemoved { .. } => None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommittedExecutionTrace {
    frames_by_id: BTreeMap<FrameId, TraceFrame>,
    events: Vec<TraceEvent>,
    snapshots: Vec<(TracePosition, SavedState)>,
    limit_exceeded: Option<AnalysisLimitExceeded>,
    applied_authorizations: Vec<CommittedAuthorization>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommittedAuthorization {
    account: Address,
    delegate: Address,
    nonce: u64,
}

impl CommittedAuthorization {
    pub(crate) const fn account(self) -> Address {
        self.account
    }

    pub(crate) const fn delegate(self) -> Address {
        self.delegate
    }

    pub(crate) const fn nonce(self) -> u64 {
        self.nonce
    }
}

pub(crate) fn filters_for_space(filters: &[LogFilter], space: Space) -> Vec<(Space, LogFilter)> {
    filters.iter().map(|&filter| (space, filter)).collect()
}

impl CommittedExecutionTrace {
    pub(crate) fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub(crate) fn applied_authorizations(&self) -> &[CommittedAuthorization] {
        &self.applied_authorizations
    }

    pub(crate) fn take_snapshots(&mut self) -> Vec<(TracePosition, SavedState)> {
        std::mem::take(&mut self.snapshots)
    }

    pub(crate) fn limit_exceeded(&self) -> Option<AnalysisLimitExceeded> {
        self.limit_exceeded
    }

    pub(crate) fn frame(&self, frame_id: FrameId) -> &TraceFrame {
        self.frames_by_id
            .get(&frame_id)
            .expect("committed trace event references a committed frame")
    }

    pub(crate) fn try_frame(&self, frame_id: FrameId) -> Option<&TraceFrame> {
        self.frames_by_id.get(&frame_id)
    }

    pub(crate) fn frames(&self) -> impl Iterator<Item = (FrameId, &TraceFrame)> {
        self.frames_by_id.iter().map(|(id, frame)| (*id, frame))
    }

    pub(crate) fn frame_is_within(&self, mut frame_id: FrameId, root_id: FrameId) -> bool {
        loop {
            if frame_id == root_id {
                return true;
            }
            let Some(parent_id) = self.frame(frame_id).parent_id else {
                return false;
            };
            frame_id = parent_id;
        }
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
    next_frame_id: usize,
    event_count: usize,
    snapshot_count: usize,
    limit_exceeded: Option<AnalysisLimitExceeded>,
}

#[derive(Debug)]
struct ActiveFrame {
    id: FrameId,
    frame_type: FrameType,
    space: Space,
    rollback_mark: JournalMark,
}

#[derive(Debug)]
struct ExecutionTraceJournal {
    frames_by_id: BTreeMap<FrameId, TraceFrame>,
    events: Vec<TraceEvent>,
    snapshots: Vec<(TracePosition, SavedState)>,
    limit_exceeded: Option<AnalysisLimitExceeded>,
    active_frames: Vec<ActiveFrame>,
    checkpoints: Vec<JournalMark>,
    next_event_position: TracePosition,
    next_frame_id: usize,
    invalid_sequence: bool,
    transaction_space: Space,
    applied_authorizations: Vec<CommittedAuthorization>,
    limits: AnalysisLimits,
}

impl ExecutionTraceJournal {
    fn new(transaction_space: Space) -> Self {
        Self {
            frames_by_id: BTreeMap::new(),
            events: Vec::new(),
            snapshots: Vec::new(),
            limit_exceeded: None,
            active_frames: Vec::new(),
            checkpoints: Vec::new(),
            next_event_position: 0,
            next_frame_id: 0,
            invalid_sequence: false,
            transaction_space,
            applied_authorizations: Vec::new(),
            limits: AnalysisLimits::default(),
        }
    }

    fn record_set_auth(&mut self, action: SetAuth) {
        // CIP-7702 authorization processing is transaction-level and is only
        // meaningful for an Ethereum/eSpace transaction.
        if self.transaction_space != Space::Ethereum || action.space != Space::Ethereum {
            return;
        }
        if action.outcome != SetAuthOutcome::Success {
            return;
        }
        let Some(account) = action.author else {
            self.invalid_sequence = true;
            return;
        };
        let Some(nonce) = u64::try_from(action.nonce).ok() else {
            self.invalid_sequence = true;
            return;
        };
        if !self.observe_fact() {
            return;
        }
        self.applied_authorizations.push(CommittedAuthorization {
            account,
            delegate: action.address,
            nonce,
        });
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
            code_hash: params.code_hash,
            action: FrameAction::Call {
                call_type: params.call_type,
                caller: params.sender,
                target: params.address,
                code_address: params.code_address,
                transferred_value: actual_transfer_value(&params.value),
                calldata: calldata.to_vec(),
            },
        };
        self.enter_frame(frame, FrameType::Call);
    }

    fn enter_create_frame(&mut self, params: &ActionParams) {
        let init_code = params
            .code
            .as_deref()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let frame = TraceFrame {
            parent_id: self.active_frames.last().map(|frame| frame.id),
            space: params.space,
            code_hash: params.code_hash,
            action: FrameAction::Create {
                creator: params.sender,
                created_address: params.address,
                actual_created_address: None,
                value: actual_transfer_value(&params.value),
                init_code: init_code.to_vec(),
            },
        };
        self.enter_frame(frame, FrameType::Create);
    }

    fn enter_frame(&mut self, frame: TraceFrame, frame_type: FrameType) {
        let rollback_mark = self.mark();
        let id = FrameId(self.next_frame_id);
        self.next_frame_id += 1;
        let space = frame.space;
        let position = self.allocate_event_position();
        if self.observe_fact() {
            self.frames_by_id.insert(id, frame);
            self.events.push(TraceEvent::FrameStart {
                position,
                frame_id: id,
            });
        }
        self.active_frames.push(ActiveFrame {
            id,
            frame_type,
            space,
            rollback_mark,
        });
    }

    fn exit_call_frame(&mut self, result: &FrameResult) {
        self.exit_frame(FrameType::Call, frame_succeeded(result));
    }

    fn exit_create_frame(&mut self, result: &FrameResult) {
        let successful = frame_succeeded(result);
        let actual_created_address = result
            .as_ref()
            .ok()
            .and_then(|result| result.create_address);
        let frame_id = self.exit_frame(FrameType::Create, successful);
        if successful
            && let Some(frame_id) = frame_id
            && let Some(frame) = self.frames_by_id.get_mut(&frame_id)
            && let FrameAction::Create {
                actual_created_address: recorded,
                ..
            } = &mut frame.action
        {
            *recorded = actual_created_address;
        }
    }

    fn exit_frame(&mut self, expected_type: FrameType, success: bool) -> Option<FrameId> {
        let Some(frame) = self.active_frames.pop() else {
            self.invalid_sequence = true;
            return None;
        };

        if frame.frame_type != expected_type {
            self.invalid_sequence = true;
            return None;
        }

        if !success {
            self.rollback_to(frame.rollback_mark);
        }

        Some(frame.id)
    }

    fn record_log(
        &mut self,
        address: Address,
        topics: &[H256],
        data: &[u8],
    ) -> Option<(TracePosition, Space)> {
        let Some(frame) = self.active_frames.last() else {
            self.invalid_sequence = true;
            return None;
        };
        let frame_id = frame.id;
        let space = frame.space;
        let position = self.allocate_event_position();
        if !self.observe_fact() {
            return None;
        }
        self.events.push(TraceEvent::Log {
            position,
            frame_id,
            address,
            topics: topics.to_vec(),
            data: data.to_vec(),
        });
        Some((position, space))
    }

    fn observe_fact(&mut self) -> bool {
        if self
            .events
            .len()
            .saturating_add(self.applied_authorizations.len())
            >= self.limits.max_observed_facts
        {
            self.limit_exceeded.get_or_insert(AnalysisLimitExceeded {
                resource: AnalysisResource::Facts,
                limit: self.limits.max_observed_facts,
            });
        }
        self.limit_exceeded.is_none()
    }

    fn record_contract_removed(&mut self, address: AddressWithSpace) {
        let position = self.allocate_event_position();
        if !self.observe_fact() {
            return;
        }
        self.events
            .push(TraceEvent::ContractRemoved { position, address });
    }

    fn snapshot(&mut self, position: TracePosition, state: &State) {
        if self.limit_exceeded.is_some() {
            return;
        }
        self.snapshots.push((position, state.snapshot()));
    }

    fn record_internal_transfer(&mut self, from: AddressPocket, to: AddressPocket, value: U256) {
        let frame_id = self.active_frames.last().map(|frame| frame.id);
        let space = self
            .active_frames
            .last()
            .map_or(self.transaction_space, |frame| frame.space);
        let position = self.allocate_event_position();
        if !self.observe_fact() {
            return;
        }
        self.events.push(TraceEvent::InternalTransfer {
            position,
            frame_id,
            space,
            from,
            to,
            value,
        });
    }

    fn record_storage_write(&mut self, address: AddressWithSpace, key: &[u8], value: U256) {
        let Some(frame_id) = self.active_frames.last().map(|frame| frame.id) else {
            self.invalid_sequence = true;
            return;
        };
        let position = self.allocate_event_position();
        if !self.observe_fact() {
            return;
        }
        self.events.push(TraceEvent::StorageWrite {
            position,
            frame_id,
            address,
            key: key.to_vec(),
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
            next_frame_id: self.next_frame_id,
            event_count: self.events.len(),
            snapshot_count: self.snapshots.len(),
            limit_exceeded: self.limit_exceeded,
        }
    }

    fn rollback_to(&mut self, mark: JournalMark) {
        if mark.event_count > self.events.len() || mark.snapshot_count > self.snapshots.len() {
            self.invalid_sequence = true;
            return;
        }
        self.events.truncate(mark.event_count);
        self.snapshots.truncate(mark.snapshot_count);
        self.limit_exceeded = mark.limit_exceeded;
        self.frames_by_id.split_off(&FrameId(mark.next_frame_id));
    }

    fn into_committed_trace(mut self) -> Option<CommittedExecutionTrace> {
        if !self.active_frames.is_empty() || !self.checkpoints.is_empty() {
            self.invalid_sequence = true;
        }
        (!self.invalid_sequence).then_some(CommittedExecutionTrace {
            frames_by_id: self.frames_by_id,
            events: self.events,
            snapshots: self.snapshots,
            limit_exceeded: self.limit_exceeded,
            applied_authorizations: self.applied_authorizations,
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

#[derive(Debug)]
pub(crate) struct ExecutionTraceObserver {
    journal: ExecutionTraceJournal,
    checkpoint_filters: Vec<(Space, LogFilter)>,
}

impl ExecutionTraceObserver {
    pub(crate) fn new(transaction_space: Space) -> Self {
        Self {
            journal: ExecutionTraceJournal::new(transaction_space),
            checkpoint_filters: Vec::new(),
        }
    }

    pub(crate) fn with_checkpoint_filters(
        mut self,
        checkpoint_filters: Vec<(Space, LogFilter)>,
        limits: AnalysisLimits,
    ) -> Self {
        self.checkpoint_filters = checkpoint_filters;
        self.journal.limits = limits;
        self
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
    fn log(&mut self, address: &Address, topics: &Vec<H256>, data: &[u8], state: &State) {
        let Some((position, space)) = self.journal.record_log(*address, topics, data) else {
            return;
        };
        if topics.first().is_some_and(|topic0| {
            let address = crate::primitive::address_from_cfx(*address);
            let topic0 = crate::primitive::b256_from_cfx(*topic0);
            self.checkpoint_filters
                .iter()
                .any(|(filter_space, filter)| {
                    *filter_space == space && filter.matches(address, topic0)
                })
        }) {
            self.journal.snapshot(position, state);
        }
    }
}

impl SetAuthTracer for ExecutionTraceObserver {
    fn record_set_auth(&mut self, set_auth_action: SetAuth) {
        self.journal.record_set_auth(set_auth_action);
    }
}
impl StorageTracer for ExecutionTraceObserver {
    fn trace_contract_removed(&mut self, address: AddressWithSpace) {
        self.journal.record_contract_removed(address);
    }

    fn trace_storage_write(&mut self, address: AddressWithSpace, key: &[u8], value: U256) {
        self.journal.record_storage_write(address, key, value);
    }
}

#[cfg(test)]
mod tests {
    use cfx_executor::executive_observer::AddressPocket;
    use cfx_types::{Address, AddressSpaceUtil, H256, Space, U256};

    use super::{ExecutionTraceJournal, FrameAction, FrameId, FrameType, TraceEvent, TraceFrame};

    #[test]
    fn rolls_back_failed_frames_and_transaction_checkpoints_without_reusing_positions() {
        let mut journal = ExecutionTraceJournal::new(Space::Ethereum);
        journal.enter_frame(create_frame(None, 1, 2), FrameType::Create);
        let _ = journal.record_log(Address::repeat_byte(2), &[H256::repeat_byte(3)], &[4]);
        journal.enter_frame(create_frame(Some(FrameId(0)), 2, 5), FrameType::Create);
        let _ = journal.record_log(Address::repeat_byte(5), &[H256::repeat_byte(6)], &[7]);
        journal.exit_frame(FrameType::Create, false);
        journal.record_internal_transfer(
            AddressPocket::GasPayment,
            AddressPocket::Balance(Address::repeat_byte(1).with_evm_space()),
            U256::from(8),
        );
        journal.exit_frame(FrameType::Create, true);

        let trace = journal
            .into_committed_trace()
            .expect("valid frame sequence should produce a committed trace");
        assert!(!trace.frames_by_id.contains_key(&FrameId(1)));
        assert_eq!(
            trace
                .events()
                .iter()
                .map(TraceEvent::position)
                .collect::<Vec<_>>(),
            [0, 1, 4]
        );
        assert!(trace.events().iter().all(|event| {
            event
                .frame_id()
                .is_none_or(|frame_id| frame_id == FrameId(0))
        }));

        let mut transaction = ExecutionTraceJournal::new(Space::Ethereum);
        transaction.checkpoint();
        transaction.record_internal_transfer(
            AddressPocket::Balance(Address::repeat_byte(1).with_evm_space()),
            AddressPocket::GasPayment,
            U256::from(9),
        );
        transaction.revert_checkpoint();
        transaction.record_internal_transfer(
            AddressPocket::GasPayment,
            AddressPocket::Balance(Address::repeat_byte(1).with_evm_space()),
            U256::from(10),
        );

        let trace = transaction
            .into_committed_trace()
            .expect("reverted checkpoint should leave a valid journal");
        assert!(matches!(
            trace.events(),
            [TraceEvent::InternalTransfer {
                position: 1,
                value,
                ..
            }] if *value == U256::from(10)
        ));
    }

    fn create_frame(parent_id: Option<FrameId>, creator: u8, created: u8) -> TraceFrame {
        TraceFrame {
            parent_id,
            space: Space::Ethereum,
            code_hash: H256::zero(),
            action: FrameAction::Create {
                creator: Address::repeat_byte(creator),
                created_address: Address::repeat_byte(created),
                actual_created_address: None,
                value: U256::zero(),
                init_code: Vec::new(),
            },
        }
    }
}

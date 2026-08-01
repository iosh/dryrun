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
const CALL_INPUT_PREFIX_LIMIT: usize = 100;

pub(crate) type Position = usize;

#[derive(Debug, PartialEq)]
pub(crate) enum Observation {
    Call {
        position: Position,
        space: Space,
        call_type: CallType,
        caller: Address,
        target: Address,
        code_address: Address,
        transferred_value: U256,
        input_len: usize,
        input_prefix: Vec<u8>,
    },
    CreateTransfer {
        position: Position,
        space: Space,
        from: Address,
        to: Address,
        value: U256,
    },
    Log {
        position: Position,
        space: Space,
        address: Address,
        topics: Vec<H256>,
        data: Vec<u8>,
    },
    InternalTransfer {
        position: Position,
        space: Space,
        from: AddressPocket,
        to: AddressPocket,
        value: U256,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum FrameKind {
    Call,
    Create,
}

#[derive(Debug)]
struct FrameCheckpoint {
    kind: FrameKind,
    entry_start: usize,
    space: Space,
}

#[derive(Debug)]
struct ObservationJournal {
    entries: Vec<Observation>,
    frames: Vec<FrameCheckpoint>,
    checkpoints: Vec<usize>,
    next_position: Position,
    invalid: bool,
    transaction_space: Space,
}

impl ObservationJournal {
    fn new(transaction_space: Space) -> Self {
        Self {
            entries: Vec::new(),
            frames: Vec::new(),
            checkpoints: Vec::new(),
            next_position: 0,
            invalid: false,
            transaction_space,
        }
    }

    fn take_position(&mut self) -> Position {
        let position = self.next_position;
        if let Some(next) = self.next_position.checked_add(1) {
            self.next_position = next;
        } else {
            self.invalid = true;
        }
        position
    }

    fn push_call_frame(&mut self, params: &ActionParams) {
        self.push_frame(FrameKind::Call, params.space);

        let input = params.data.as_deref().unwrap_or_default();

        let position = self.take_position();
        self.entries.push(Observation::Call {
            position,
            space: params.space,
            call_type: params.call_type,
            caller: params.sender,
            target: params.address,
            code_address: params.code_address,
            transferred_value: actual_transfer_value(&params.value),
            input_len: input.len(),
            input_prefix: input
                .iter()
                .copied()
                .take(call_input_bytes_to_capture(
                    params.space,
                    params.address,
                    params.code_address,
                ))
                .collect(),
        });
    }

    fn push_create_frame(&mut self, params: &ActionParams) {
        self.push_frame(FrameKind::Create, params.space);

        let position = self.take_position();
        self.entries.push(Observation::CreateTransfer {
            position,
            space: params.space,
            from: params.sender,
            to: params.address,
            value: actual_transfer_value(&params.value),
        });
    }

    fn push_frame(&mut self, kind: FrameKind, space: Space) {
        self.frames.push(FrameCheckpoint {
            kind,
            entry_start: self.entries.len(),
            space,
        });
    }

    fn finish_call_frame(&mut self, result: &FrameResult) {
        self.finish_frame(FrameKind::Call, frame_succeeded(result));
    }

    fn finish_create_frame(&mut self, result: &FrameResult) {
        self.finish_frame(FrameKind::Create, frame_succeeded(result));
    }

    fn finish_frame(&mut self, expected_kind: FrameKind, success: bool) {
        let Some(frame) = self.frames.pop() else {
            self.invalid = true;
            return;
        };

        if frame.kind != expected_kind {
            self.invalid = true;
            return;
        }

        if !success {
            self.entries.truncate(frame.entry_start);
        }
    }

    fn record_log(&mut self, address: Address, topics: &[H256], data: &[u8]) {
        let Some(space) = self.frames.last().map(|frame| frame.space) else {
            self.invalid = true;
            return;
        };

        let position = self.take_position();
        self.entries.push(Observation::Log {
            position,
            space,
            address,
            topics: topics.to_vec(),
            data: data.to_vec(),
        });
    }

    fn record_internal_transfer(&mut self, from: AddressPocket, to: AddressPocket, value: U256) {
        let space = self
            .frames
            .last()
            .map_or(self.transaction_space, |frame| frame.space);
        let position = self.take_position();
        self.entries.push(Observation::InternalTransfer {
            position,
            space,
            from,
            to,
            value,
        });
    }

    fn trace_checkpoint(&mut self) {
        self.checkpoints.push(self.entries.len());
    }

    fn trace_checkpoint_discard(&mut self) {
        if !self.frames.is_empty() {
            self.invalid = true;
        }
        if self.checkpoints.pop().is_none() {
            self.invalid = true;
        }
    }

    fn trace_checkpoint_revert(&mut self) {
        if !self.frames.is_empty() {
            self.invalid = true;
        }
        let Some(entry_start) = self.checkpoints.pop() else {
            self.invalid = true;
            return;
        };
        self.entries.truncate(entry_start);
    }

    fn finish(mut self) -> Option<Vec<Observation>> {
        if !self.frames.is_empty() || !self.checkpoints.is_empty() {
            self.invalid = true;
        }
        (!self.invalid).then_some(self.entries)
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

fn call_input_bytes_to_capture(space: Space, target: Address, code_address: Address) -> usize {
    let sponsor_contract =
        cfx_parameters::internal_contract_addresses::SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS;
    if space == Space::Native && (target == sponsor_contract || code_address == sponsor_contract) {
        usize::MAX
    } else {
        CALL_INPUT_PREFIX_LIMIT
    }
}

#[derive(Debug)]
pub(crate) struct ObservationObserver {
    journal: ObservationJournal,
}

impl ObservationObserver {
    pub(crate) fn new(transaction_space: Space) -> Self {
        Self {
            journal: ObservationJournal::new(transaction_space),
        }
    }
}

pub(crate) struct ObservationKey;

impl typemap::Key for ObservationKey {
    type Value = Vec<Observation>;
}

impl AsTracer for ObservationObserver {
    fn as_tracer<'a>(&'a mut self) -> Box<dyn 'a + TracerTrait> {
        Box::new(self)
    }
}

impl DrainTrace for ObservationObserver {
    fn drain_trace(self, map: &mut ShareDebugMap) {
        if let Some(observations) = self.journal.finish() {
            map.insert::<ObservationKey>(observations);
        }
    }
}

impl CallTracer for ObservationObserver {
    fn record_call(&mut self, params: &ActionParams) {
        self.journal.push_call_frame(params);
    }

    fn record_call_result(&mut self, result: &FrameResult) {
        self.journal.finish_call_frame(result);
    }

    fn record_create(&mut self, params: &ActionParams) {
        self.journal.push_create_frame(params);
    }

    fn record_create_result(&mut self, result: &FrameResult) {
        self.journal.finish_create_frame(result);
    }
}

impl CheckpointTracer for ObservationObserver {
    fn trace_checkpoint(&mut self) {
        self.journal.trace_checkpoint();
    }

    fn trace_checkpoint_discard(&mut self) {
        self.journal.trace_checkpoint_discard();
    }

    fn trace_checkpoint_revert(&mut self) {
        self.journal.trace_checkpoint_revert();
    }
}

impl InternalTransferTracer for ObservationObserver {
    fn trace_internal_transfer(&mut self, from: AddressPocket, to: AddressPocket, value: U256) {
        self.journal.record_internal_transfer(from, to, value);
    }
}

impl OpcodeTracer for ObservationObserver {
    fn log(&mut self, address: &Address, topics: &Vec<H256>, data: &[u8]) {
        self.journal.record_log(*address, topics, data);
    }
}

impl SetAuthTracer for ObservationObserver {}
impl StorageTracer for ObservationObserver {}

#[cfg(test)]
mod tests {
    use cfx_executor::observer::AddressPocket;
    use cfx_types::{Address, AddressWithSpace, Space, U256};
    use cfx_vm_types::ActionValue;
    use typemap::ShareDebugMap;

    use super::{
        FrameKind, Observation, ObservationJournal, ObservationKey, ObservationObserver, Position,
        actual_transfer_value,
    };

    fn address(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn push_frame(journal: &mut ObservationJournal) {
        journal.push_frame(FrameKind::Call, Space::Native);
    }

    #[test]
    fn frame_revert_keeps_gap() {
        let mut journal = ObservationJournal::new(Space::Native);
        push_frame(&mut journal);
        journal.record_log(address(1), &[], &[]);

        push_frame(&mut journal);
        journal.record_internal_transfer(
            AddressPocket::Balance(AddressWithSpace {
                address: address(2),
                space: Space::Native,
            }),
            AddressPocket::MintBurn,
            U256::from(7),
        );
        journal.finish_frame(FrameKind::Call, false);
        journal.record_log(address(3), &[], &[]);
        journal.finish_frame(FrameKind::Call, true);

        let observations = journal.finish().expect("valid journal");
        assert_eq!(observations.len(), 2);
        assert_eq!(observation_position(&observations[0]), 0);
        assert_eq!(observation_position(&observations[1]), 2);
    }

    #[test]
    fn transaction_revert_keeps_gap() {
        let mut journal = ObservationJournal::new(Space::Native);
        journal.record_internal_transfer(
            AddressPocket::MintBurn,
            AddressPocket::GasPayment,
            U256::from(1),
        );
        journal.trace_checkpoint();
        journal.record_internal_transfer(
            AddressPocket::MintBurn,
            AddressPocket::GasPayment,
            U256::from(2),
        );
        journal.trace_checkpoint_revert();
        journal.record_internal_transfer(
            AddressPocket::MintBurn,
            AddressPocket::GasPayment,
            U256::from(3),
        );

        let observations = journal.finish().expect("valid journal");
        assert_eq!(observations.len(), 2);
        assert_eq!(observation_position(&observations[0]), 0);
        assert_eq!(observation_position(&observations[1]), 2);
    }

    #[test]
    fn call_value_and_drain() {
        assert_eq!(
            actual_transfer_value(&ActionValue::Apparent(U256::from(9))),
            U256::zero()
        );
        assert_eq!(
            actual_transfer_value(&ActionValue::Transfer(U256::from(7))),
            U256::from(7)
        );

        let observer = ObservationObserver::new(Space::Native);
        let mut map = ShareDebugMap::custom();
        cfx_executor::executive_observer::DrainTrace::drain_trace(observer, &mut map);
        assert_eq!(map.remove::<ObservationKey>(), Some(Vec::new()));
    }

    fn observation_position(observation: &Observation) -> Position {
        match observation {
            Observation::Call { position, .. }
            | Observation::CreateTransfer { position, .. }
            | Observation::Log { position, .. }
            | Observation::InternalTransfer { position, .. } => *position,
        }
    }
}

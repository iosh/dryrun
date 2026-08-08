use alloy::primitives::{Address, B256, Bytes, Log, U256};
use revm::{
    Inspector,
    context::ContextTr,
    context_interface::LocalContextTr,
    interpreter::{
        CallInput, CallInputs, CallOutcome, CreateInputs, CreateOutcome, InstructionResult,
        InterpreterTypes,
    },
};

// transferFrom(address,address,uint256) is a 4-byte selector plus three ABI words.
const CALL_INPUT_PREFIX_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvmExecutionObservation {
    Call {
        caller: Address,
        target: Address,
        value: U256,
        input_len: usize,
        input_prefix: Bytes,
    },
    CreateTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestruct {
        contract: Address,
        target: Address,
        amount: U256,
    },
    Log {
        address: Address,
        topics: Vec<B256>,
        data: Bytes,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ObservationJournalEntry {
    Committed(EvmExecutionObservation),
    // CREATE transfers need a placeholder because the created address is only
    // known when the frame finishes.
    PendingCreateTransfer { from: Address, amount: U256 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameCheckpoint {
    checkpoint: usize,
    pending_create_transfer_index: Option<usize>,
}

#[derive(Debug, Default)]
struct ObservationJournal {
    checkpoints: Vec<FrameCheckpoint>,
    entries: Vec<ObservationJournalEntry>,
}

impl ObservationJournal {
    fn push_call_frame(&mut self, call: Option<EvmExecutionObservation>) {
        let checkpoint = self.entries.len();
        self.checkpoints.push(FrameCheckpoint {
            checkpoint,
            pending_create_transfer_index: None,
        });

        if let Some(call) = call {
            self.entries.push(ObservationJournalEntry::Committed(call));
        }
    }

    fn push_create_frame(&mut self, from: Address, amount: U256) {
        let checkpoint = self.entries.len();
        let pending_create_transfer_index = if amount.is_zero() {
            None
        } else {
            let index = self.entries.len();
            self.entries
                .push(ObservationJournalEntry::PendingCreateTransfer { from, amount });
            Some(index)
        };

        self.checkpoints.push(FrameCheckpoint {
            checkpoint,
            pending_create_transfer_index,
        });
    }

    fn pop_frame(&mut self, success: bool, created_address: Option<Address>) {
        let Some(frame) = self.checkpoints.pop() else {
            return;
        };

        // Reverting a frame also discards every observation produced by its
        // descendants because they all live past the same checkpoint.
        if !success {
            self.entries.truncate(frame.checkpoint);
            return;
        }

        let Some(index) = frame.pending_create_transfer_index else {
            return;
        };
        let Some(to) = created_address else {
            return;
        };

        let ObservationJournalEntry::PendingCreateTransfer { from, amount } = &self.entries[index]
        else {
            return;
        };

        self.entries[index] =
            ObservationJournalEntry::Committed(EvmExecutionObservation::CreateTransfer {
                from: *from,
                to,
                amount: *amount,
            });
    }

    fn record_observation(&mut self, observation: EvmExecutionObservation) {
        self.entries
            .push(ObservationJournalEntry::Committed(observation));
    }

    fn record_log_parts(&mut self, address: Address, topics: &[B256], data: &Bytes) {
        self.record_observation(EvmExecutionObservation::Log {
            address,
            topics: topics.to_vec(),
            data: data.clone(),
        });
    }
}

#[derive(Debug, Default)]
pub(crate) struct EvmExecutionObserver {
    journal: ObservationJournal,
}

impl EvmExecutionObserver {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn take_observations(&mut self) -> Vec<EvmExecutionObservation> {
        std::mem::take(&mut self.journal.entries)
            .into_iter()
            .filter_map(|entry| match entry {
                ObservationJournalEntry::Committed(observation) => Some(observation),
                ObservationJournalEntry::PendingCreateTransfer { .. } => None,
            })
            .collect()
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for EvmExecutionObserver
where
    CTX: ContextTr,
    INTR: InterpreterTypes,
{
    fn log(&mut self, _context: &mut CTX, log: Log) {
        self.journal
            .record_log_parts(log.address, log.data.topics(), &log.data.data);
    }

    fn call(&mut self, context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.journal.push_call_frame(observed_call(context, inputs));
        None
    }

    fn call_end(&mut self, _context: &mut CTX, _inputs: &CallInputs, outcome: &mut CallOutcome) {
        self.journal
            .pop_frame(is_success(outcome.instruction_result()), None);
    }

    fn create(&mut self, _context: &mut CTX, inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.journal
            .push_create_frame(inputs.caller(), inputs.value());
        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        self.journal
            .pop_frame(is_success(outcome.instruction_result()), outcome.address);
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        if value.is_zero() {
            return;
        }

        self.journal
            .record_observation(EvmExecutionObservation::SelfDestruct {
                contract,
                target,
                amount: value,
            });
    }
}

fn observed_call<CTX>(context: &CTX, inputs: &CallInputs) -> Option<EvmExecutionObservation>
where
    CTX: ContextTr,
{
    if !inputs.scheme.is_call() {
        return None;
    }

    Some(EvmExecutionObservation::Call {
        caller: inputs.caller,
        target: inputs.target_address,
        value: inputs.transfer_value().unwrap_or_default(),
        input_len: inputs.input.len(),
        input_prefix: call_input_prefix(context, &inputs.input),
    })
}

fn call_input_prefix<CTX>(context: &CTX, input: &CallInput) -> Bytes
where
    CTX: ContextTr,
{
    let prefix_len = input.len().min(CALL_INPUT_PREFIX_LIMIT);

    match input {
        CallInput::Bytes(bytes) => copy_input_prefix(bytes),
        // Internal CALL input points into Revm shared memory and must be copied
        // before the child frame can overwrite that buffer.
        CallInput::SharedBuffer(range) => {
            let prefix = context
                .local()
                .shared_memory_buffer_slice(range.start..range.start.saturating_add(prefix_len))
                .map(|bytes| copy_input_prefix(&bytes))
                .unwrap_or_default();
            debug_assert_eq!(prefix.len(), prefix_len);
            prefix
        }
    }
}

fn copy_input_prefix(input: &[u8]) -> Bytes {
    Bytes::copy_from_slice(&input[..input.len().min(CALL_INPUT_PREFIX_LIMIT)])
}

fn is_success(result: &InstructionResult) -> bool {
    result.is_ok()
}

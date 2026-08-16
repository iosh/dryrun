use alloy::primitives::{Address, B256, Bytes, Log, U256};
use revm::{
    Inspector,
    context::ContextTr,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, InstructionResult, InterpreterTypes,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvmExecutionObservation {
    Call {
        caller: Address,
        target: Address,
        value: U256,
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

    fn call(&mut self, _context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.journal.push_call_frame(observed_call(inputs));
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

fn observed_call(inputs: &CallInputs) -> Option<EvmExecutionObservation> {
    if !inputs.scheme.is_call() {
        return None;
    }

    Some(EvmExecutionObservation::Call {
        caller: inputs.caller,
        target: inputs.target_address,
        value: inputs.transfer_value().unwrap_or_default(),
    })
}

fn is_success(result: &InstructionResult) -> bool {
    result.is_ok()
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, U256};

    use super::{EvmExecutionObservation, ObservationJournal, ObservationJournalEntry};

    #[test]
    fn reverting_parent_discards_its_nested_observations() {
        let retained = observed_call(1, 2, 3);
        let mut journal = ObservationJournal::default();
        journal.record_observation(retained.clone());
        journal.push_call_frame(Some(observed_call(4, 5, 6)));
        journal.push_call_frame(Some(observed_call(7, 8, 9)));
        journal.record_observation(observed_call(10, 11, 12));
        journal.pop_frame(true, None);
        journal.pop_frame(false, None);

        assert_eq!(
            journal.entries,
            vec![ObservationJournalEntry::Committed(retained)]
        );
    }

    #[test]
    fn create_transfer_is_committed_only_with_a_successful_address() {
        let from = Address::repeat_byte(1);
        let created_address = Address::repeat_byte(2);
        let amount = U256::from(3);
        let nested_call = observed_call(4, 5, 6);

        let mut successful = ObservationJournal::default();
        successful.push_create_frame(from, amount);
        successful.record_observation(nested_call.clone());
        successful.pop_frame(true, Some(created_address));
        assert_eq!(
            successful.entries,
            vec![
                ObservationJournalEntry::Committed(EvmExecutionObservation::CreateTransfer {
                    from,
                    to: created_address,
                    amount,
                },),
                ObservationJournalEntry::Committed(nested_call.clone()),
            ]
        );

        let mut failed = ObservationJournal::default();
        failed.push_create_frame(from, amount);
        failed.record_observation(nested_call);
        failed.pop_frame(false, None);
        assert!(failed.entries.is_empty());
    }

    fn observed_call(caller: u8, target: u8, value: u64) -> EvmExecutionObservation {
        EvmExecutionObservation::Call {
            caller: Address::repeat_byte(caller),
            target: Address::repeat_byte(target),
            value: U256::from(value),
        }
    }
}

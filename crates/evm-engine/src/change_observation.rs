use alloy_primitives::{Address, B256, Bytes, Log, U256};
use revm::{
    Inspector,
    context::ContextTr,
    interpreter::{
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, InstructionResult,
        InterpreterTypes,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Observation {
    NativeTransfer {
        from: Address,
        to: Address,
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
    Committed(Observation),
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
    fn push_call_frame(&mut self, native_transfer: Option<Observation>) {
        let checkpoint = self.entries.len();
        self.checkpoints.push(FrameCheckpoint {
            checkpoint,
            pending_create_transfer_index: None,
        });

        if let Some(change) = native_transfer {
            self.entries
                .push(ObservationJournalEntry::Committed(change));
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

        self.entries[index] = ObservationJournalEntry::Committed(Observation::NativeTransfer {
            from: *from,
            to,
            amount: *amount,
        });
    }

    fn record_observation(&mut self, observation: Observation) {
        self.entries
            .push(ObservationJournalEntry::Committed(observation));
    }

    fn record_log_parts(&mut self, address: Address, topics: &[B256], data: &Bytes) {
        self.record_observation(Observation::Log {
            address,
            topics: topics.to_vec(),
            data: data.clone(),
        });
    }

    fn into_observations(self) -> Vec<Observation> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry {
                ObservationJournalEntry::Committed(observation) => Some(observation),
                ObservationJournalEntry::PendingCreateTransfer { .. } => None,
            })
            .collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChangeObservationInspector {
    journal: ObservationJournal,
}

impl ChangeObservationInspector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn into_observations(self) -> Vec<Observation> {
        self.journal.into_observations()
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for ChangeObservationInspector
where
    CTX: ContextTr,
    INTR: InterpreterTypes,
{
    fn log(&mut self, _context: &mut CTX, log: Log) {
        self.journal
            .record_log_parts(log.address, log.data.topics(), &log.data.data);
    }

    fn call(&mut self, _context: &mut CTX, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.journal.push_call_frame(observed_call_transfer(inputs));
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
            .record_observation(Observation::NativeTransfer {
                from: contract,
                to: target,
                amount: value,
            });
    }
}

// Native value transfers are observed from CALL-like execution semantics rather
// than from logs because they are an EVM effect, not a contract event.
fn observed_call_transfer(inputs: &CallInputs) -> Option<Observation> {
    let amount = inputs.transfer_value()?;

    if amount.is_zero() {
        return None;
    }

    if !matches!(inputs.scheme, CallScheme::Call) {
        return None;
    }

    Some(Observation::NativeTransfer {
        from: inputs.caller,
        to: inputs.target_address,
        amount,
    })
}

fn is_success(result: &InstructionResult) -> bool {
    result.is_ok()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, U256};

    use super::{Observation, ObservationJournal};

    fn address(value: &str) -> Address {
        Address::from_str(value).expect("address")
    }

    fn topic(value: u8) -> B256 {
        B256::repeat_byte(value)
    }

    #[test]
    fn keeps_native_transfers_and_logs_in_real_observed_order() {
        let mut journal = ObservationJournal::default();
        let from = address("0x1111111111111111111111111111111111111111");
        let callee = address("0x2222222222222222222222222222222222222222");
        let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let topics = vec![topic(0xaa), topic(0xbb)];
        let data = Bytes::from(vec![0x01, 0x02]);

        journal.push_call_frame(Some(Observation::NativeTransfer {
            from,
            to: callee,
            amount: U256::from(1_u64),
        }));
        journal.record_observation(Observation::Log {
            address: token,
            topics: topics.clone(),
            data: data.clone(),
        });
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observations(),
            vec![
                Observation::NativeTransfer {
                    from,
                    to: callee,
                    amount: U256::from(1_u64),
                },
                Observation::Log {
                    address: token,
                    topics,
                    data,
                },
            ]
        );
    }

    #[test]
    fn truncates_reverted_branch_with_all_descendants() {
        let mut journal = ObservationJournal::default();
        let root_from = address("0x1111111111111111111111111111111111111111");
        let root_to = address("0x2222222222222222222222222222222222222222");
        let reverted_to = address("0x3333333333333333333333333333333333333333");
        let sibling_to = address("0x4444444444444444444444444444444444444444");

        journal.push_call_frame(Some(Observation::NativeTransfer {
            from: root_from,
            to: root_to,
            amount: U256::from(1_u64),
        }));

        journal.push_call_frame(Some(Observation::NativeTransfer {
            from: root_to,
            to: reverted_to,
            amount: U256::from(2_u64),
        }));
        journal.record_observation(Observation::NativeTransfer {
            from: reverted_to,
            to: sibling_to,
            amount: U256::from(3_u64),
        });
        journal.pop_frame(false, None);

        journal.record_observation(Observation::NativeTransfer {
            from: root_to,
            to: sibling_to,
            amount: U256::from(4_u64),
        });
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observations(),
            vec![
                Observation::NativeTransfer {
                    from: root_from,
                    to: root_to,
                    amount: U256::from(1_u64),
                },
                Observation::NativeTransfer {
                    from: root_to,
                    to: sibling_to,
                    amount: U256::from(4_u64),
                },
            ]
        );
    }

    #[test]
    fn keeps_create_transfer_at_original_position_after_success() {
        let mut journal = ObservationJournal::default();
        let creator = address("0x1111111111111111111111111111111111111111");
        let created = address("0x2222222222222222222222222222222222222222");
        let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let topics = vec![topic(0xcc)];
        let data = Bytes::from(vec![0x03]);

        journal.push_create_frame(creator, U256::from(1_u64));
        journal.record_observation(Observation::Log {
            address: token,
            topics: topics.clone(),
            data: data.clone(),
        });
        journal.pop_frame(true, Some(created));

        assert_eq!(
            journal.into_observations(),
            vec![
                Observation::NativeTransfer {
                    from: creator,
                    to: created,
                    amount: U256::from(1_u64),
                },
                Observation::Log {
                    address: token,
                    topics,
                    data,
                },
            ]
        );
    }

    #[test]
    fn drops_create_transfer_and_nested_changes_on_revert() {
        let mut journal = ObservationJournal::default();
        let creator = address("0x1111111111111111111111111111111111111111");
        let other = address("0x2222222222222222222222222222222222222222");

        journal.push_create_frame(creator, U256::from(1_u64));
        journal.record_observation(Observation::NativeTransfer {
            from: other,
            to: creator,
            amount: U256::from(2_u64),
        });
        journal.pop_frame(false, None);

        assert!(journal.into_observations().is_empty());
    }
}

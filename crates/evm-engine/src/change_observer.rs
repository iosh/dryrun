use alloy_primitives::{Address, B256, Bytes, Log, U256, keccak256};
use revm::{
    Inspector,
    context::ContextTr,
    interpreter::{
        CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome, InstructionResult,
        InterpreterTypes,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObservedChange {
    NativeTransfer {
        from: Address,
        to: Address,
        amount: U256,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        amount: U256,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChangeJournalEntry {
    Committed(ObservedChange),
    PendingCreateTransfer {
        from: Address,
        amount: U256,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameCheckpoint {
    checkpoint: usize,
    pending_create_transfer_index: Option<usize>,
}

#[derive(Debug, Default)]
struct ChangeJournal {
    checkpoints: Vec<FrameCheckpoint>,
    entries: Vec<ChangeJournalEntry>,
}

impl ChangeJournal {
    fn push_call_frame(&mut self, native_transfer: Option<ObservedChange>) {
        let checkpoint = self.entries.len();
        self.checkpoints.push(FrameCheckpoint {
            checkpoint,
            pending_create_transfer_index: None,
        });

        if let Some(change) = native_transfer {
            self.entries.push(ChangeJournalEntry::Committed(change));
        }
    }

    fn push_create_frame(&mut self, from: Address, amount: U256) {
        let checkpoint = self.entries.len();
        let pending_create_transfer_index = if amount.is_zero() {
            None
        } else {
            let index = self.entries.len();
            self.entries
                .push(ChangeJournalEntry::PendingCreateTransfer { from, amount });
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

        let ChangeJournalEntry::PendingCreateTransfer { from, amount } =
            self.entries[index].clone()
        else {
            return;
        };

        self.entries[index] = ChangeJournalEntry::Committed(ObservedChange::NativeTransfer {
            from,
            to,
            amount,
        });
    }

    fn record_change(&mut self, change: ObservedChange) {
        self.entries.push(ChangeJournalEntry::Committed(change));
    }

    fn record_log_parts(&mut self, address: Address, topics: &[B256], data: &Bytes) {
        let Some(change) = extract_erc20_transfer_from_parts(address, topics, data) else {
            return;
        };

        self.record_change(change);
    }

    fn into_observed_changes(self) -> Vec<ObservedChange> {
        self.entries
            .into_iter()
            .filter_map(|entry| match entry {
                ChangeJournalEntry::Committed(change) => Some(change),
                ChangeJournalEntry::PendingCreateTransfer { .. } => None,
            })
            .collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChangeObserverInspector {
    journal: ChangeJournal,
}

impl ChangeObserverInspector {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn into_observed_changes(self) -> Vec<ObservedChange> {
        self.journal.into_observed_changes()
    }
}

impl<CTX, INTR> Inspector<CTX, INTR> for ChangeObserverInspector
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

        self.journal.record_change(ObservedChange::NativeTransfer {
            from: contract,
            to: target,
            amount: value,
        });
    }
}

fn observed_call_transfer(inputs: &CallInputs) -> Option<ObservedChange> {
    let amount = inputs.transfer_value()?;

    if amount.is_zero() {
        return None;
    }

    if !matches!(inputs.scheme, CallScheme::Call) {
        return None;
    }

    Some(ObservedChange::NativeTransfer {
        from: inputs.caller,
        to: inputs.target_address,
        amount,
    })
}

fn is_success(result: &InstructionResult) -> bool {
    result.is_ok()
}

fn erc20_transfer_topic0() -> B256 {
    keccak256("Transfer(address,address,uint256)".as_bytes())
}

fn is_zero_padded_address_topic(topic: &B256) -> bool {
    topic.as_slice()[..12].iter().all(|&byte| byte == 0)
}

fn extract_erc20_transfer_from_parts(
    address: Address,
    topics: &[B256],
    data: &Bytes,
) -> Option<ObservedChange> {
    if topics.len() != 3 {
        return None;
    }

    if topics[0] != erc20_transfer_topic0() {
        return None;
    }

    if data.len() != 32 {
        return None;
    }

    if !is_zero_padded_address_topic(&topics[1]) || !is_zero_padded_address_topic(&topics[2]) {
        return None;
    }

    Some(ObservedChange::Erc20Transfer {
        contract_address: address,
        from: Address::from_word(topics[1]),
        to: Address::from_word(topics[2]),
        amount: U256::from_be_slice(data.as_ref()),
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, U256};

    use super::{ChangeJournal, ObservedChange};

    fn address(value: &str) -> Address {
        Address::from_str(value).expect("address")
    }

    #[test]
    fn keeps_mixed_changes_in_real_observed_order() {
        let mut journal = ChangeJournal::default();
        let from = address("0x1111111111111111111111111111111111111111");
        let callee = address("0x2222222222222222222222222222222222222222");
        let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let user = address("0x3333333333333333333333333333333333333333");

        journal.push_call_frame(Some(ObservedChange::NativeTransfer {
            from,
            to: callee,
            amount: U256::from(1_u64),
        }));
        journal.record_change(ObservedChange::Erc20Transfer {
            contract_address: token,
            from: callee,
            to: user,
            amount: U256::from(2_u64),
        });
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observed_changes(),
            vec![
                ObservedChange::NativeTransfer {
                    from,
                    to: callee,
                    amount: U256::from(1_u64),
                },
                ObservedChange::Erc20Transfer {
                    contract_address: token,
                    from: callee,
                    to: user,
                    amount: U256::from(2_u64),
                },
            ]
        );
    }

    #[test]
    fn truncates_reverted_branch_with_all_descendants() {
        let mut journal = ChangeJournal::default();
        let root_from = address("0x1111111111111111111111111111111111111111");
        let root_to = address("0x2222222222222222222222222222222222222222");
        let reverted_to = address("0x3333333333333333333333333333333333333333");
        let sibling_to = address("0x4444444444444444444444444444444444444444");

        journal.push_call_frame(Some(ObservedChange::NativeTransfer {
            from: root_from,
            to: root_to,
            amount: U256::from(1_u64),
        }));

        journal.push_call_frame(Some(ObservedChange::NativeTransfer {
            from: root_to,
            to: reverted_to,
            amount: U256::from(2_u64),
        }));
        journal.record_change(ObservedChange::NativeTransfer {
            from: reverted_to,
            to: sibling_to,
            amount: U256::from(3_u64),
        });
        journal.pop_frame(false, None);

        journal.record_change(ObservedChange::NativeTransfer {
            from: root_to,
            to: sibling_to,
            amount: U256::from(4_u64),
        });
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observed_changes(),
            vec![
                ObservedChange::NativeTransfer {
                    from: root_from,
                    to: root_to,
                    amount: U256::from(1_u64),
                },
                ObservedChange::NativeTransfer {
                    from: root_to,
                    to: sibling_to,
                    amount: U256::from(4_u64),
                },
            ]
        );
    }

    #[test]
    fn keeps_create_transfer_at_original_position_after_success() {
        let mut journal = ChangeJournal::default();
        let creator = address("0x1111111111111111111111111111111111111111");
        let created = address("0x2222222222222222222222222222222222222222");
        let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");

        journal.push_create_frame(creator, U256::from(1_u64));
        journal.record_change(ObservedChange::Erc20Transfer {
            contract_address: token,
            from: created,
            to: creator,
            amount: U256::from(2_u64),
        });
        journal.pop_frame(true, Some(created));

        assert_eq!(
            journal.into_observed_changes(),
            vec![
                ObservedChange::NativeTransfer {
                    from: creator,
                    to: created,
                    amount: U256::from(1_u64),
                },
                ObservedChange::Erc20Transfer {
                    contract_address: token,
                    from: created,
                    to: creator,
                    amount: U256::from(2_u64),
                },
            ]
        );
    }

    #[test]
    fn drops_create_transfer_and_nested_changes_on_revert() {
        let mut journal = ChangeJournal::default();
        let creator = address("0x1111111111111111111111111111111111111111");
        let other = address("0x2222222222222222222222222222222222222222");

        journal.push_create_frame(creator, U256::from(1_u64));
        journal.record_change(ObservedChange::NativeTransfer {
            from: other,
            to: creator,
            amount: U256::from(2_u64),
        });
        journal.pop_frame(false, None);

        assert!(journal.into_observed_changes().is_empty());
    }
}

use alloy_primitives::{Address, B256, Bytes, Log, U256};
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
pub(crate) enum Observation {
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
    fn push_call_frame(&mut self, call: Option<Observation>) {
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

        self.entries[index] = ObservationJournalEntry::Committed(Observation::CreateTransfer {
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

        self.journal.record_observation(Observation::SelfDestruct {
            contract,
            target,
            amount: value,
        });
    }
}

fn observed_call<CTX>(context: &CTX, inputs: &CallInputs) -> Option<Observation>
where
    CTX: ContextTr,
{
    if !inputs.scheme.is_call() {
        return None;
    }

    Some(Observation::Call {
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{Address, B256, Bytes, TxKind, U256};
    use revm::{
        Context, InspectEvm, MainBuilder, MainContext,
        context::TxEnv,
        database::InMemoryDB,
        state::{AccountInfo, Bytecode, bytecode::opcode},
    };

    use super::{Observation, ObservationJournal};

    fn address(value: &str) -> Address {
        Address::from_str(value).expect("address")
    }

    fn topic(value: u8) -> B256 {
        B256::repeat_byte(value)
    }

    fn call_observation(caller: Address, target: Address, value: u64) -> Observation {
        Observation::Call {
            caller,
            target,
            value: U256::from(value),
            input_len: 0,
            input_prefix: Bytes::new(),
        }
    }

    fn selfdestruct_observation(contract: Address, target: Address, amount: u64) -> Observation {
        Observation::SelfDestruct {
            contract,
            target,
            amount: U256::from(amount),
        }
    }

    fn insert_contract(db: &mut InMemoryDB, contract: Address, code: Vec<u8>, balance: u64) {
        db.insert_account_info(
            contract,
            AccountInfo::default()
                .with_balance(U256::from(balance))
                .with_nonce(1)
                .with_code(Bytecode::new_raw(Bytes::from(code))),
        );
    }

    fn inspect_call(
        db: InMemoryDB,
        caller: Address,
        target: Address,
        input: Bytes,
    ) -> Vec<Observation> {
        inspect_call_with_value(db, caller, target, U256::ZERO, input)
    }

    fn inspect_call_with_value(
        mut db: InMemoryDB,
        caller: Address,
        target: Address,
        value: U256,
        input: Bytes,
    ) -> Vec<Observation> {
        db.insert_account_info(
            caller,
            AccountInfo::default().with_balance(U256::from(1_000_000_000_u64)),
        );

        let mut evm = Context::mainnet()
            .with_db(db)
            .build_mainnet_with_inspector(super::ChangeObservationInspector::new());

        evm.inspect_one_tx(
            TxEnv::builder()
                .caller(caller)
                .kind(TxKind::Call(target))
                .value(value)
                .data(input)
                .gas_limit(500_000)
                .build()
                .expect("valid test transaction"),
        )
        .expect("test execution");

        std::mem::take(&mut evm.inspector).into_observations()
    }

    fn internal_call_code(target: Address, input_len: u8, revert_after_call: bool) -> Vec<u8> {
        let mut code = vec![
            // CALLDATACOPY(0, 0, input_len)
            opcode::PUSH1,
            input_len,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::CALLDATACOPY,
            // CALL(target, value=0, input=memory[0..input_len], output=empty)
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            input_len,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x00,
            opcode::PUSH20,
        ];
        code.extend_from_slice(target.as_ref());
        code.extend_from_slice(&[opcode::PUSH2, 0xff, 0xff, opcode::CALL]);

        if revert_after_call {
            code.extend_from_slice(&[opcode::PUSH1, 0x00, opcode::PUSH1, 0x00, opcode::REVERT]);
        } else {
            code.push(opcode::STOP);
        }

        code
    }

    fn create_code(create2: bool) -> Vec<u8> {
        let mut code = Vec::new();

        // CREATE2 consumes a salt below the common size, offset, and value
        // arguments. Zero-initialized memory supplies one STOP byte of initcode.
        if create2 {
            code.extend_from_slice(&[opcode::PUSH1, 0x01]);
        }

        code.extend_from_slice(&[
            opcode::PUSH1,
            0x01,
            opcode::PUSH1,
            0x00,
            opcode::PUSH1,
            0x01,
            if create2 {
                opcode::CREATE2
            } else {
                opcode::CREATE
            },
            opcode::STOP,
        ]);
        code
    }

    #[test]
    fn keeps_call_and_log_in_observed_order() {
        let mut journal = ObservationJournal::default();
        let from = address("0x1111111111111111111111111111111111111111");
        let callee = address("0x2222222222222222222222222222222222222222");
        let token = address("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
        let topics = vec![topic(0xaa), topic(0xbb)];
        let data = Bytes::from(vec![0x01, 0x02]);
        let call = call_observation(from, callee, 1);

        journal.push_call_frame(Some(call.clone()));
        journal.record_observation(Observation::Log {
            address: token,
            topics: topics.clone(),
            data: data.clone(),
        });
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observations(),
            vec![
                call,
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
        let root_call = call_observation(root_from, root_to, 1);
        let surviving_selfdestruct = selfdestruct_observation(root_to, sibling_to, 4);

        journal.push_call_frame(Some(root_call.clone()));

        journal.push_call_frame(Some(call_observation(root_to, reverted_to, 2)));
        journal.record_observation(selfdestruct_observation(reverted_to, sibling_to, 3));
        journal.pop_frame(false, None);

        journal.record_observation(surviving_selfdestruct.clone());
        journal.pop_frame(true, None);

        assert_eq!(
            journal.into_observations(),
            vec![root_call, surviving_selfdestruct]
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
                Observation::CreateTransfer {
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
        journal.record_observation(selfdestruct_observation(other, creator, 2));
        journal.pop_frame(false, None);

        assert!(journal.into_observations().is_empty());
    }

    #[test]
    fn captures_top_level_call_length_and_bounded_input() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let target = address("0x2222222222222222222222222222222222222222");

        for input_len in [99, 100, 101] {
            let mut db = InMemoryDB::default();
            insert_contract(&mut db, target, vec![opcode::STOP], 0);
            let input = Bytes::from(vec![0xab; input_len]);
            let observations =
                inspect_call_with_value(db, caller, target, U256::from(7_u64), input.clone());

            assert_eq!(
                observations,
                vec![Observation::Call {
                    caller,
                    target,
                    value: U256::from(7_u64),
                    input_len,
                    input_prefix: input.slice(..input_len.min(100)),
                }]
            );
        }
    }

    #[test]
    fn copies_internal_call_shared_buffer_and_preserves_log_order() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let parent = address("0x2222222222222222222222222222222222222222");
        let child = address("0x3333333333333333333333333333333333333333");
        let input = Bytes::from(vec![0xcd; 101]);
        let mut db = InMemoryDB::default();
        insert_contract(&mut db, parent, internal_call_code(child, 101, false), 0);
        insert_contract(
            &mut db,
            child,
            vec![
                opcode::PUSH1,
                0x00,
                opcode::PUSH1,
                0x00,
                opcode::LOG0,
                opcode::STOP,
            ],
            0,
        );

        let observations = inspect_call(db, caller, parent, input.clone());
        let prefix = input.slice(..100);

        assert_eq!(
            observations,
            vec![
                Observation::Call {
                    caller,
                    target: parent,
                    value: U256::ZERO,
                    input_len: 101,
                    input_prefix: prefix.clone(),
                },
                Observation::Call {
                    caller: parent,
                    target: child,
                    value: U256::ZERO,
                    input_len: 101,
                    input_prefix: prefix,
                },
                Observation::Log {
                    address: child,
                    topics: Vec::new(),
                    data: Bytes::new(),
                },
            ]
        );
    }

    #[test]
    fn drops_successful_internal_call_and_log_when_top_level_reverts() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let parent = address("0x2222222222222222222222222222222222222222");
        let child = address("0x3333333333333333333333333333333333333333");
        let mut db = InMemoryDB::default();
        insert_contract(&mut db, parent, internal_call_code(child, 0, true), 0);
        insert_contract(
            &mut db,
            child,
            vec![
                opcode::PUSH1,
                0x00,
                opcode::PUSH1,
                0x00,
                opcode::LOG0,
                opcode::STOP,
            ],
            0,
        );

        let observations = inspect_call(db, caller, parent, Bytes::new());

        assert!(observations.is_empty());
    }

    #[test]
    fn records_create_and_create2_value_at_their_frame_position() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let factory = address("0x2222222222222222222222222222222222222222");

        for create2 in [false, true] {
            let mut db = InMemoryDB::default();
            insert_contract(&mut db, factory, create_code(create2), 10);
            let observations = inspect_call(db, caller, factory, Bytes::new());

            assert_eq!(observations.len(), 2);
            assert_eq!(observations[0], call_observation(caller, factory, 0));
            let Observation::CreateTransfer { from, to, amount } = observations[1] else {
                panic!("expected create transfer observation");
            };
            assert_eq!(from, factory);
            assert_ne!(to, Address::ZERO);
            assert_eq!(amount, U256::from(1_u64));
        }
    }

    #[test]
    fn records_selfdestruct_separately_after_the_top_level_call() {
        let caller = address("0x1111111111111111111111111111111111111111");
        let contract = address("0x2222222222222222222222222222222222222222");
        let beneficiary = address("0x3333333333333333333333333333333333333333");
        let mut code = vec![opcode::PUSH20];
        code.extend_from_slice(beneficiary.as_ref());
        code.push(opcode::SELFDESTRUCT);
        let mut db = InMemoryDB::default();
        insert_contract(&mut db, contract, code, 9);

        let observations = inspect_call(db, caller, contract, Bytes::new());

        assert_eq!(
            observations,
            vec![
                call_observation(caller, contract, 0),
                selfdestruct_observation(contract, beneficiary, 9),
            ]
        );
    }
}

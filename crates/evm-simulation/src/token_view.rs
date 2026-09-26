use crate::{
    EvmCallKind, EvmFrameAction, EvmReadCallOutcome, EvmStateReader, EvmTransactionExecution,
};
use alloy_primitives::{Address, B256, Bytes, U256};
use contract_standards::analysis::{AnalysisError, view::*};

pub(crate) struct EvmTokenView<'a> {
    pub(crate) execution: &'a EvmTransactionExecution,
}

impl ContractState for EvmStateReader {
    fn native_balance(&self, account: Address) -> Result<U256, AnalysisError> {
        self.read_account(account)
            .map(|state| state.balance())
            .map_err(AnalysisError::state)
    }
    fn code(&self, account: Address) -> Result<Bytes, AnalysisError> {
        self.code(account).map_err(AnalysisError::state)
    }
    fn storage(&self, account: Address, slot: B256) -> Result<B256, AnalysisError> {
        self.storage_word(account, slot)
            .map_err(AnalysisError::state)
    }
    fn read_call(&self, target: Address, input: Bytes) -> Result<ReadCallOutcome, AnalysisError> {
        Ok(
            match self
                .read_call(target, input)
                .map_err(AnalysisError::state)?
            {
                EvmReadCallOutcome::Success(output) => ReadCallOutcome::Success(output),
                EvmReadCallOutcome::Reverted(output) => ReadCallOutcome::Reverted(output),
                EvmReadCallOutcome::Halted { .. } => ReadCallOutcome::Halted,
            },
        )
    }
}

impl TokenView for EvmTokenView<'_> {
    fn committed_frames(&self) -> Box<dyn Iterator<Item = Frame<'_>> + '_> {
        Box::new(self.execution.committed_frames().iter().map(|frame| Frame {
            id: frame.id().index(),
            parent: frame.parent().map(|id| id.index()),
            position: frame.position().index(),
            code_hash: frame.code_hash(),
            action: match frame.action() {
                EvmFrameAction::Call {
                    kind,
                    caller,
                    target,
                    bytecode_address,
                    value,
                    input,
                } => FrameAction::Call {
                    kind: match kind {
                        EvmCallKind::Call => CallKind::Call,
                        EvmCallKind::CallCode => CallKind::CallCode,
                        EvmCallKind::DelegateCall => CallKind::DelegateCall,
                        EvmCallKind::StaticCall => CallKind::StaticCall,
                    },
                    caller: *caller,
                    target: *target,
                    bytecode_address: *bytecode_address,
                    value: *value,
                    input,
                },
                EvmFrameAction::Create {
                    caller,
                    value,
                    init_code,
                    created_address,
                } => FrameAction::Create {
                    caller: *caller,
                    address: created_address.expect("committed create address was verified"),
                    value: *value,
                    init_code,
                },
            },
        }))
    }
    fn committed_logs(&self) -> Box<dyn Iterator<Item = CommittedLog<'_>> + '_> {
        Box::new(
            self.execution
                .committed_logs()
                .iter()
                .map(|entry| CommittedLog {
                    position: entry.position().index(),
                    frame_id: entry.frame_id().index(),
                    log: LogRef {
                        address: entry.log().address,
                        topics: entry.log().data.topics().into(),
                        data: &entry.log().data.data,
                    },
                }),
        )
    }
    fn log_checkpoints(&self) -> Box<dyn Iterator<Item = LogCheckpoint<'_>> + '_> {
        Box::new(
            self.execution
                .log_checkpoints()
                .map(|checkpoint| LogCheckpoint {
                    position: checkpoint.position().index(),
                    frame_id: checkpoint.frame_id().index(),
                    log: LogRef {
                        address: checkpoint.log().address,
                        topics: checkpoint.log().data.topics().into(),
                        data: &checkpoint.log().data.data,
                    },
                    states: StatePair {
                        previous: checkpoint.previous_state(),
                        current: checkpoint.state(),
                    },
                }),
        )
    }
    fn storage_writes(&self) -> Box<dyn Iterator<Item = StorageWrite> + '_> {
        Box::new(
            self.execution
                .storage_writes()
                .iter()
                .map(|write| StorageWrite {
                    position: write.position().index(),
                    frame_id: write.frame_id().index(),
                    address: write.address(),
                    slot: Some(write.slot().into()),
                    value: write.value(),
                }),
        )
    }
    fn initial(&self) -> &dyn ContractState {
        self.execution.state().initial()
    }
    fn finalized(&self) -> &dyn ContractState {
        self.execution.state().finalized()
    }
}

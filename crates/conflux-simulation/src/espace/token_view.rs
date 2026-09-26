use crate::espace::{
    EspaceCallKind, EspaceExecutedTransaction, EspaceExecutionSpace, EspaceFrameAction,
    EspaceReadCallOutcome, EspaceStateReader,
};
use alloy_primitives::{Address, B256, Bytes, U256};
use contract_standards::analysis::{AnalysisError, view::*};

pub(crate) struct EspaceTokenView<'a> {
    pub(crate) execution: &'a EspaceExecutedTransaction,
}

impl ContractState for EspaceStateReader {
    fn native_balance(&self, account: Address) -> Result<U256, AnalysisError> {
        self.read_account(account)
            .map(|state| state.balance())
            .map_err(AnalysisError::state)
    }
    fn code(&self, account: Address) -> Result<Bytes, AnalysisError> {
        self.read_account(account)
            .map(|state| state.code().cloned().unwrap_or_default())
            .map_err(AnalysisError::state)
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
                EspaceReadCallOutcome::Success(output) => ReadCallOutcome::Success(output),
                EspaceReadCallOutcome::Reverted(output) => ReadCallOutcome::Reverted(output),
                EspaceReadCallOutcome::Failed => ReadCallOutcome::Halted,
            },
        )
    }
}

impl TokenView for EspaceTokenView<'_> {
    fn committed_frames(&self) -> Box<dyn Iterator<Item = Frame<'_>> + '_> {
        Box::new(
            self.execution
                .committed_frames()
                .iter()
                .filter(|frame| frame.space() == EspaceExecutionSpace::Espace)
                .map(|frame| Frame {
                    id: frame.id().index(),
                    parent: frame.parent().map(|id| id.index()),
                    position: frame.position().index(),
                    code_hash: Some(frame.code_hash()),
                    action: match frame.action() {
                        EspaceFrameAction::Call {
                            kind,
                            caller,
                            target,
                            code_address: bytecode_address,
                            value,
                            calldata: input,
                        } => FrameAction::Call {
                            kind: match kind {
                                EspaceCallKind::Call => CallKind::Call,
                                EspaceCallKind::CallCode => CallKind::CallCode,
                                EspaceCallKind::DelegateCall => CallKind::DelegateCall,
                                EspaceCallKind::StaticCall => CallKind::StaticCall,
                            },
                            caller: *caller,
                            target: *target,
                            bytecode_address: *bytecode_address,
                            value: *value,
                            input,
                        },
                        EspaceFrameAction::Create {
                            creator,
                            value,
                            init_code,
                            actual_address,
                            ..
                        } => FrameAction::Create {
                            caller: *creator,
                            address: *actual_address,
                            value: *value,
                            init_code,
                        },
                    },
                }),
        )
    }
    fn committed_logs(&self) -> Box<dyn Iterator<Item = CommittedLog<'_>> + '_> {
        Box::new(
            self.execution
                .committed_logs()
                .iter()
                .filter(|log| log.space() == EspaceExecutionSpace::Espace)
                .map(|log| CommittedLog {
                    position: log.position().index(),
                    frame_id: log.frame_id().index(),
                    log: LogRef {
                        address: log.address(),
                        topics: log.topics().into(),
                        data: log.data(),
                    },
                }),
        )
    }
    fn log_checkpoints(&self) -> Box<dyn Iterator<Item = LogCheckpoint<'_>> + '_> {
        Box::new(
            self.execution
                .log_checkpoints()
                .filter(|checkpoint| checkpoint.log().space() == EspaceExecutionSpace::Espace)
                .map(|checkpoint| LogCheckpoint {
                    position: checkpoint.position().index(),
                    frame_id: checkpoint.frame_id().index(),
                    log: LogRef {
                        address: checkpoint.log().address(),
                        topics: checkpoint.log().topics().into(),
                        data: checkpoint.log().data(),
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
                .filter(|write| write.space() == EspaceExecutionSpace::Espace)
                .map(|write| StorageWrite {
                    position: write.position().index(),
                    frame_id: write.frame_id().index(),
                    address: write.address(),
                    slot: write.key().as_ref().try_into().ok().map(B256::new),
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

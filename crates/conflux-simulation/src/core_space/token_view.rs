use super::{CoreSpaceExecutedTransaction, CoreSpaceStateReader};
use crate::{
    execution::{
        FrameAction as ConfluxFrameAction, ReadCallOutcome as ConfluxReadCallOutcome, TraceEvent,
    },
    primitive::{address_from_cfx, address_to_cfx, b256_from_cfx, u256_from_cfx},
};
use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_types::{AddressWithSpace, Space};
use contract_standards::analysis::{AnalysisError, view::*};

pub(crate) struct CoreTokenView<'a> {
    pub(crate) execution: &'a CoreSpaceExecutedTransaction,
    pub(crate) space: Space,
}

impl ContractState for CoreSpaceStateReader {
    fn native_balance(&self, account: Address) -> Result<U256, AnalysisError> {
        self.balance_in(AddressWithSpace {
            address: address_to_cfx(account),
            space: self.space,
        })
        .map_err(AnalysisError::state)
    }
    fn code(&self, account: Address) -> Result<Bytes, AnalysisError> {
        self.code(AddressWithSpace {
            address: address_to_cfx(account),
            space: self.space,
        })
        .map(|code| code.unwrap_or_default())
        .map_err(AnalysisError::state)
    }
    fn storage(&self, account: Address, slot: B256) -> Result<B256, AnalysisError> {
        self.storage_word(
            AddressWithSpace {
                address: address_to_cfx(account),
                space: self.space,
            },
            slot.as_slice(),
        )
        .map(|value| B256::from(u256_from_cfx(value)))
        .map_err(AnalysisError::state)
    }
    fn read_call(&self, target: Address, input: Bytes) -> Result<ReadCallOutcome, AnalysisError> {
        Ok(
            match self
                .read_call_in(
                    AddressWithSpace {
                        address: address_to_cfx(target),
                        space: self.space,
                    },
                    input,
                )
                .map_err(AnalysisError::state)?
            {
                ConfluxReadCallOutcome::Success(output) => ReadCallOutcome::Success(output),
                ConfluxReadCallOutcome::Reverted(output) => ReadCallOutcome::Reverted(output),
                ConfluxReadCallOutcome::Failed => ReadCallOutcome::Halted,
            },
        )
    }
}

impl TokenView for CoreTokenView<'_> {
    fn committed_frames(&self) -> Box<dyn Iterator<Item = Frame<'_>> + '_> {
        Box::new(self.execution.trace().events().iter().filter_map(|event| {
            let TraceEvent::FrameStart { position, frame_id } = event else {
                return None;
            };
            let frame = self.execution.trace().frame(*frame_id);
            if frame.space != self.space {
                return None;
            }
            Some(Frame {
                id: frame_id.index(),
                parent: frame.parent_id.map(|id| id.index()),
                position: *position,
                code_hash: Some(b256_from_cfx(frame.code_hash)),
                action: match &frame.action {
                    ConfluxFrameAction::Call {
                        call_type,
                        caller,
                        target,
                        code_address,
                        transferred_value,
                        calldata,
                        ..
                    } => FrameAction::Call {
                        kind: match call_type {
                            cfx_vm_types::CallType::Call => CallKind::Call,
                            cfx_vm_types::CallType::CallCode => CallKind::CallCode,
                            cfx_vm_types::CallType::DelegateCall => CallKind::DelegateCall,
                            cfx_vm_types::CallType::StaticCall => CallKind::StaticCall,
                            cfx_vm_types::CallType::None => CallKind::Call,
                        },
                        caller: address_from_cfx(*caller),
                        target: address_from_cfx(*target),
                        bytecode_address: address_from_cfx(*code_address),
                        value: u256_from_cfx(*transferred_value),
                        input: calldata,
                    },
                    ConfluxFrameAction::Create {
                        creator,
                        actual_created_address,
                        value,
                        init_code,
                        ..
                    } => FrameAction::Create {
                        caller: address_from_cfx(*creator),
                        address: address_from_cfx(
                            actual_created_address.expect("committed create address was verified"),
                        ),
                        value: u256_from_cfx(*value),
                        init_code,
                    },
                },
            })
        }))
    }
    fn committed_logs(&self) -> Box<dyn Iterator<Item = CommittedLog<'_>> + '_> {
        Box::new(self.execution.trace().events().iter().filter_map(|event| {
            let TraceEvent::Log {
                position,
                frame_id,
                address,
                topics,
                data,
            } = event
            else {
                return None;
            };
            if self.execution.trace().frame(*frame_id).space != self.space {
                return None;
            }
            Some(CommittedLog {
                position: *position,
                frame_id: frame_id.index(),
                log: LogRef {
                    address: address_from_cfx(*address),
                    topics: topics
                        .iter()
                        .copied()
                        .map(b256_from_cfx)
                        .collect::<Vec<_>>()
                        .into(),
                    data,
                },
            })
        }))
    }
    fn log_checkpoints(&self) -> Box<dyn Iterator<Item = LogCheckpoint<'_>> + '_> {
        let space = match self.space {
            Space::Native => super::CoreSpaceExecutionSpace::Core,
            Space::Ethereum => super::CoreSpaceExecutionSpace::Espace,
        };
        Box::new(
            self.execution
                .log_checkpoints_in(self.space)
                .filter(move |checkpoint| checkpoint.space() == space)
                .map(|checkpoint| LogCheckpoint {
                    position: checkpoint.position().index(),
                    frame_id: checkpoint.frame_id().index(),
                    log: LogRef {
                        address: checkpoint.address(),
                        topics: checkpoint.topics().collect::<Vec<_>>().into(),
                        data: checkpoint.data(),
                    },
                    states: StatePair {
                        previous: checkpoint.previous_state(),
                        current: checkpoint.state(),
                    },
                }),
        )
    }
    fn storage_writes(&self) -> Box<dyn Iterator<Item = StorageWrite> + '_> {
        Box::new(self.execution.trace().events().iter().filter_map(|event| {
            let TraceEvent::StorageWrite {
                position,
                frame_id,
                address,
                key,
                value,
            } = event
            else {
                return None;
            };
            (address.space == self.space).then(|| StorageWrite {
                position: *position,
                frame_id: frame_id.index(),
                address: address_from_cfx(address.address),
                slot: key.as_slice().try_into().ok().map(B256::new),
                value: u256_from_cfx(*value),
            })
        }))
    }
    fn initial(&self) -> &dyn ContractState {
        self.execution.state().initial_in(self.space)
    }
    fn finalized(&self) -> &dyn ContractState {
        self.execution.state().finalized_in(self.space)
    }
}

use alloy::{
    primitives::{Address, U256},
    sol,
    sol_types::SolEvent,
};
use cfx_types::Space;
use contract_standards::Erc20Metadata;

use crate::{
    espace::{EspaceCommittedLog, EspaceExecutionSpace},
    execution::{CommittedExecutionTrace, FrameId, TraceEvent},
    primitive::{address_from_cfx, b256_from_cfx},
};

use super::{ChangeOccurrence, EspaceChange};

sol! {
    event Deposit(address indexed account, uint256 amount);
    event Withdrawal(address indexed account, uint256 amount);
}

#[derive(Debug)]
pub(super) struct WrappedNativeOccurrence {
    position: usize,
    contract_address: Address,
    event: WrappedNativeEvent,
}

#[derive(Debug)]
enum WrappedNativeEvent {
    Deposit { account: Address, raw_amount: U256 },
    Withdrawal { account: Address, raw_amount: U256 },
}

impl WrappedNativeOccurrence {
    pub(super) const fn position(&self) -> usize {
        self.position
    }

    pub(super) const fn contract_address(&self) -> Address {
        self.contract_address
    }

    pub(super) fn into_change(self, metadata: Erc20Metadata) -> ChangeOccurrence {
        let change = match self.event {
            WrappedNativeEvent::Deposit {
                account,
                raw_amount,
            } => EspaceChange::WrappedNativeDeposit {
                contract_address: self.contract_address,
                account,
                raw_amount,
                metadata,
            },
            WrappedNativeEvent::Withdrawal {
                account,
                raw_amount,
            } => EspaceChange::WrappedNativeWithdrawal {
                contract_address: self.contract_address,
                account,
                raw_amount,
                metadata,
            },
        };
        ChangeOccurrence::new(self.position, change)
    }
}

pub(super) fn decode_wrapped_native_occurrences(
    logs: &[EspaceCommittedLog],
    contract_address: Address,
) -> Vec<WrappedNativeOccurrence> {
    logs.iter()
        .filter(|log| {
            log.space() == EspaceExecutionSpace::Espace && log.address() == contract_address
        })
        .filter_map(|log| {
            decode_wrapped_native_log(log.topics(), log.data()).map(|event| {
                WrappedNativeOccurrence {
                    position: log.position().index(),
                    contract_address,
                    event,
                }
            })
        })
        .collect()
}

pub(super) fn decode_wrapped_native_occurrences_in_scope(
    trace: &CommittedExecutionTrace,
    contract_address: Address,
    includes_frame: impl Fn(FrameId) -> bool,
) -> Vec<WrappedNativeOccurrence> {
    trace
        .events()
        .iter()
        .filter_map(|trace_event| {
            let TraceEvent::Log {
                position,
                frame_id,
                address,
                topics,
                data,
            } = trace_event
            else {
                return None;
            };
            if trace.frame(*frame_id).space != Space::Ethereum {
                return None;
            }
            if !includes_frame(*frame_id) {
                return None;
            }
            if address_from_cfx(*address) != contract_address
                || topics.len() != 2
                || data.len() != 32
            {
                return None;
            }
            let topics = topics
                .iter()
                .copied()
                .map(b256_from_cfx)
                .collect::<Vec<_>>();
            let event = decode_wrapped_native_log(&topics, data)?;

            Some(WrappedNativeOccurrence {
                position: *position,
                contract_address,
                event,
            })
        })
        .collect()
}

fn decode_wrapped_native_log(
    topics: &[alloy::primitives::B256],
    data: &[u8],
) -> Option<WrappedNativeEvent> {
    if topics.len() != 2 || data.len() != 32 {
        return None;
    }
    if topics[0] == Deposit::SIGNATURE_HASH {
        let event = Deposit::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
        Some(WrappedNativeEvent::Deposit {
            account: event.account,
            raw_amount: event.amount,
        })
    } else if topics[0] == Withdrawal::SIGNATURE_HASH {
        let event = Withdrawal::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
        Some(WrappedNativeEvent::Withdrawal {
            account: event.account,
            raw_amount: event.amount,
        })
    } else {
        None
    }
}

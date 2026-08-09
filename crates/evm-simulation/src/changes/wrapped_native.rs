use alloy::{
    primitives::{Address, U256},
    sol,
    sol_types::SolEvent,
};
use contract_standards::Erc20Metadata;

use crate::{EvmChange, EvmExecutionObservation, changes::ChangeOccurrence};

sol! {
    event Deposit(address indexed account, uint256 amount);
    event Withdrawal(address indexed account, uint256 amount);
}

#[derive(Debug)]
pub(super) struct WrappedNativeOccurrence {
    observation_index: usize,
    contract_address: Address,
    event: WrappedNativeEvent,
}

#[derive(Debug)]
enum WrappedNativeEvent {
    Deposit { account: Address, raw_amount: U256 },
    Withdrawal { account: Address, raw_amount: U256 },
}

impl WrappedNativeOccurrence {
    pub(super) const fn observation_index(&self) -> usize {
        self.observation_index
    }

    pub(super) const fn contract_address(&self) -> Address {
        self.contract_address
    }

    pub(super) fn into_change(self, metadata: Erc20Metadata) -> ChangeOccurrence {
        let change = match self.event {
            WrappedNativeEvent::Deposit {
                account,
                raw_amount,
            } => EvmChange::WrappedNativeDeposit {
                contract_address: self.contract_address,
                account,
                raw_amount,
                metadata,
            },
            WrappedNativeEvent::Withdrawal {
                account,
                raw_amount,
            } => EvmChange::WrappedNativeWithdrawal {
                contract_address: self.contract_address,
                account,
                raw_amount,
                metadata,
            },
        };

        ChangeOccurrence::new(self.observation_index, change)
    }
}

pub(super) fn decode_wrapped_native_occurrences(
    observations: &[EvmExecutionObservation],
    contract_address: Address,
) -> Vec<WrappedNativeOccurrence> {
    observations
        .iter()
        .enumerate()
        .filter_map(|(observation_index, observation)| {
            let EvmExecutionObservation::Log {
                address,
                topics,
                data,
            } = observation
            else {
                return None;
            };

            if *address != contract_address || topics.len() != 2 || data.len() != 32 {
                return None;
            }

            let event = if topics[0] == Deposit::SIGNATURE_HASH {
                let deposit =
                    Deposit::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
                WrappedNativeEvent::Deposit {
                    account: deposit.account,
                    raw_amount: deposit.amount,
                }
            } else if topics[0] == Withdrawal::SIGNATURE_HASH {
                let withdrawal =
                    Withdrawal::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
                WrappedNativeEvent::Withdrawal {
                    account: withdrawal.account,
                    raw_amount: withdrawal.amount,
                }
            } else {
                return None;
            };

            Some(WrappedNativeOccurrence {
                observation_index,
                contract_address,
                event,
            })
        })
        .collect()
}

use alloy::{
    primitives::{Address, U256},
    sol,
    sol_types::SolEvent,
};
use cfx_types::Space;
use contract_standards::Erc20Metadata;

use crate::{
    execution::Observation,
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
    observations: &[Observation],
    contract_address: Address,
) -> Vec<WrappedNativeOccurrence> {
    observations
        .iter()
        .filter_map(|observation| {
            let Observation::Log {
                position,
                space: Space::Ethereum,
                address,
                topics,
                data,
            } = observation
            else {
                return None;
            };
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
            let event = if topics[0] == Deposit::SIGNATURE_HASH {
                let event = Deposit::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
                WrappedNativeEvent::Deposit {
                    account: event.account,
                    raw_amount: event.amount,
                }
            } else if topics[0] == Withdrawal::SIGNATURE_HASH {
                let event =
                    Withdrawal::decode_raw_log_validate(topics.iter().copied(), data).ok()?;
                WrappedNativeEvent::Withdrawal {
                    account: event.account,
                    raw_amount: event.amount,
                }
            } else {
                return None;
            };

            Some(WrappedNativeOccurrence {
                position: *position,
                contract_address,
                event,
            })
        })
        .collect()
}

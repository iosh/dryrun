use std::collections::HashSet;

use alloy::primitives::{Address, B256, Log, U256};
use contract_standards::{DecodedStandardEvent, DecodedStandardLog, decode_standard_log};

use crate::{
    EvmChangeDerivationError,
    execution::{EvmExecutionPosition, EvmSemanticLogOccurrence, EvmTransactionExecution},
};

use super::{DEPOSIT_TOPIC0, WITHDRAWAL_TOPIC0, error::token_change_error_at};

#[derive(Debug)]
pub(super) enum ObservedTokenEvent {
    Standard {
        occurrence: EvmSemanticLogOccurrence,
        decoded: DecodedStandardLog<Address>,
    },
    Wrapped {
        occurrence: EvmSemanticLogOccurrence,
        contract: Address,
        account: Address,
        amount: U256,
        direction: WrappedOperation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WrappedOperation {
    Deposit,
    Withdrawal,
}

#[derive(Debug)]
pub(super) struct TokenEventSequence {
    pub(super) events: Vec<ObservedTokenEvent>,
    pub(super) pairs: Vec<WrappedEventPair>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WrappedEventPair {
    pub(super) transfer_event_index: usize,
    pub(super) wrapped_event_index: usize,
    pub(super) position: EvmExecutionPosition,
    pub(super) direction: WrappedOperation,
    pub(super) amount: U256,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum WrappedStateEffect {
    Exact,
    Unchanged,
}

#[derive(Debug, Default)]
pub(super) struct WrappedPairProof {
    exact: u8,
    unchanged: u8,
}

impl WrappedPairProof {
    pub(super) fn record(&mut self, evidence: WrappedStateEffect) {
        match evidence {
            WrappedStateEffect::Exact => self.exact += 1,
            WrappedStateEffect::Unchanged => self.unchanged += 1,
        }
    }

    pub(super) fn proves_single_transition(&self, amount: U256) -> bool {
        if amount == U256::ZERO {
            self.exact == 2 && self.unchanged == 0
        } else {
            self.exact == 1 && self.unchanged == 1
        }
    }
}

pub(super) fn collect_token_events(
    execution: &EvmTransactionExecution,
    wrapped_native_token: Option<Address>,
) -> Result<TokenEventSequence, EvmChangeDerivationError> {
    let occurrences = execution.semantic_log_occurrences()?;
    let mut events = Vec::new();

    for occurrence in occurrences {
        let log = occurrence.log();
        let Some(topic0) = log.data.topics().first() else {
            continue;
        };

        if wrapped_native_token.is_some_and(|address| address == log.address)
            && (*topic0 == *DEPOSIT_TOPIC0 || *topic0 == *WITHDRAWAL_TOPIC0)
        {
            let (account, amount, direction) = decode_wrapped_log(log)
                .map_err(|error| token_change_error_at(occurrence.position(), error))?;
            events.push(ObservedTokenEvent::Wrapped {
                occurrence: occurrence.clone(),
                contract: log.address,
                account,
                amount,
                direction,
            });
            continue;
        }

        let Some(decoded) =
            decode_standard_log(log.address, log.data.topics(), &log.data.data, |address| {
                address
            })
            .ok()
            .flatten()
        else {
            continue;
        };
        events.push(ObservedTokenEvent::Standard {
            occurrence: occurrence.clone(),
            decoded,
        });
    }

    Ok(TokenEventSequence {
        pairs: pair_wrapped_events(&events),
        events,
    })
}

fn pair_wrapped_events(events: &[ObservedTokenEvent]) -> Vec<WrappedEventPair> {
    let mut used_wrapped = HashSet::new();
    let mut pairs = Vec::new();

    for (transfer_event_index, event) in events.iter().enumerate() {
        let ObservedTokenEvent::Standard {
            occurrence,
            decoded,
        } = event
        else {
            continue;
        };
        let DecodedStandardEvent::Erc20Transfer {
            token,
            from,
            to,
            amount,
        } = decoded.event()
        else {
            continue;
        };

        let (direction, account) = if *from == Address::ZERO && *to != Address::ZERO {
            (WrappedOperation::Deposit, *to)
        } else if *to == Address::ZERO && *from != Address::ZERO {
            (WrappedOperation::Withdrawal, *from)
        } else {
            continue;
        };

        let Some((wrapped_event_index, wrapped)) =
            events.iter().enumerate().find(|(index, item)| {
                if used_wrapped.contains(index) {
                    return false;
                }
                let ObservedTokenEvent::Wrapped {
                    occurrence: wrapped_occurrence,
                    contract: wrapped_contract,
                    account: wrapped_account,
                    amount: wrapped_amount,
                    direction: wrapped_direction,
                } = item
                else {
                    return false;
                };
                occurrence.frame_id() == wrapped_occurrence.frame_id()
                    && *wrapped_contract == *token
                    && *wrapped_account == account
                    && *wrapped_amount == *amount
                    && *wrapped_direction == direction
            })
        else {
            continue;
        };

        used_wrapped.insert(wrapped_event_index);
        let ObservedTokenEvent::Wrapped { occurrence, .. } = wrapped else {
            unreachable!("pair search only returns wrapped events");
        };
        pairs.push(WrappedEventPair {
            transfer_event_index,
            wrapped_event_index,
            position: occurrence.position(),
            direction,
            amount: *amount,
        });
    }

    pairs
}

fn decode_wrapped_log(log: &Log) -> Result<(Address, U256, WrappedOperation), &'static str> {
    let topics = log.data.topics();
    if topics.len() != 2 || log.data.data.len() != 32 {
        return Err("malformed wrapped-native event");
    }
    let account = indexed_address(&topics[1])?;
    let amount = U256::from_be_slice(&log.data.data);
    let direction = if topics[0] == *DEPOSIT_TOPIC0 {
        WrappedOperation::Deposit
    } else if topics[0] == *WITHDRAWAL_TOPIC0 {
        WrappedOperation::Withdrawal
    } else {
        return Err("unsupported wrapped-native event topic");
    };
    Ok((account, amount, direction))
}

fn indexed_address(topic: &B256) -> Result<Address, &'static str> {
    if topic.as_slice()[..12].iter().any(|byte| *byte != 0) {
        return Err("indexed address is not zero padded");
    }
    Ok(Address::from_word(*topic))
}

pub(super) fn nonzero_address(address: Address) -> Option<Address> {
    (address != Address::ZERO).then_some(address)
}

use std::collections::HashSet;

use alloy::primitives::{Address, U256};
use contract_standards::{
    DecodedStandardEvent, DecodedStandardLog, MetadataCall, decode_standard_log, metadata_calls,
};

use crate::espace::{
    EspaceChangesError, EspaceExecutedTransaction, EspaceExecutionPosition, EspaceExecutionSpace,
    EspaceSemanticLogOccurrence,
    changes::wrapped_native::{WrappedNativeEvent, decode_wrapped_native_log},
};

use super::error::token_change_error_at;

#[derive(Debug)]
pub(super) enum ObservedTokenEvent<'a> {
    Standard {
        occurrence: EspaceSemanticLogOccurrence<'a>,
        decoded: DecodedStandardLog<Address>,
    },
    Wrapped {
        occurrence: EspaceSemanticLogOccurrence<'a>,
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
pub(super) struct TokenEventSequence<'a> {
    pub(super) events: Vec<ObservedTokenEvent<'a>>,
    pub(super) pairs: Vec<WrappedEventPair>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WrappedEventPair {
    pub(super) transfer_event_index: usize,
    pub(super) wrapped_event_index: usize,
    pub(super) position: EspaceExecutionPosition,
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

pub(super) fn collect_token_events<'a>(
    execution: &'a EspaceExecutedTransaction,
    wrapped_native_token: Address,
) -> Result<TokenEventSequence<'a>, EspaceChangesError> {
    let occurrences = execution
        .semantic_log_occurrences()
        .map_err(|error| EspaceChangesError::derivation("token", error))?;
    let mut events = Vec::new();

    for occurrence in occurrences {
        let log = occurrence.log();
        if log.space() != EspaceExecutionSpace::Espace {
            continue;
        }

        if log.address() == wrapped_native_token {
            let wrapped = decode_wrapped_native_log(log.topics(), log.data())
                .map_err(|error| token_change_error_at(occurrence.position(), error))?;
            if let Some(event) = wrapped {
                let (account, amount, direction) = match event {
                    WrappedNativeEvent::Deposit {
                        account,
                        raw_amount,
                    } => (account, raw_amount, WrappedOperation::Deposit),
                    WrappedNativeEvent::Withdrawal {
                        account,
                        raw_amount,
                    } => (account, raw_amount, WrappedOperation::Withdrawal),
                };
                events.push(ObservedTokenEvent::Wrapped {
                    occurrence,
                    contract: wrapped_native_token,
                    account,
                    amount,
                    direction,
                });
                continue;
            }
        }

        let Some(decoded) =
            decode_standard_log(log.address(), log.topics(), log.data(), |address| address)
                .ok()
                .flatten()
        else {
            continue;
        };
        events.push(ObservedTokenEvent::Standard {
            occurrence,
            decoded,
        });
    }

    Ok(TokenEventSequence {
        pairs: pair_wrapped_events(&events),
        events,
    })
}

fn pair_wrapped_events(events: &[ObservedTokenEvent<'_>]) -> Vec<WrappedEventPair> {
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
                    contract,
                    account: wrapped_account,
                    amount: wrapped_amount,
                    direction: wrapped_direction,
                } = item
                else {
                    return false;
                };
                occurrence.frame_id() == wrapped_occurrence.frame_id()
                    && *contract == *token
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

pub(super) fn required_metadata_calls(
    events: &[ObservedTokenEvent<'_>],
) -> Vec<MetadataCall<Address>> {
    let decoded = events.iter().filter_map(|event| match event {
        ObservedTokenEvent::Standard { decoded, .. } => Some(decoded),
        ObservedTokenEvent::Wrapped { .. } => None,
    });
    let mut calls = metadata_calls(decoded);
    let mut seen = calls.iter().cloned().collect::<HashSet<_>>();
    for event in events {
        let ObservedTokenEvent::Wrapped { contract, .. } = event else {
            continue;
        };
        for call in [
            MetadataCall::Name {
                contract_address: *contract,
            },
            MetadataCall::Symbol {
                contract_address: *contract,
            },
            MetadataCall::Decimals {
                contract_address: *contract,
            },
        ] {
            if seen.insert(call.clone()) {
                calls.push(call);
            }
        }
    }
    calls
}

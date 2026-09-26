mod call_evidence;
mod error;
mod event_verification;
mod events;
mod sequence_verification;
mod state_queries;
mod verified_changes;

use std::collections::HashMap;

use alloy::primitives::Address;

use crate::espace::{EspaceAnalysisError, EspaceExecutedTransaction, EspaceStateAccess};

use super::super::EspaceStandardChange;

use self::{
    error::state_mismatch_at,
    events::{ObservedTokenEvent, collect_token_events},
    sequence_verification::{verify_event, verify_final_state},
    verified_changes::VerifiedTokenChange,
};

pub(crate) use events::WrappedOperation;

pub(crate) enum VerifiedChange {
    Standard {
        position: usize,
        change: EspaceStandardChange,
    },
    Wrapped {
        position: usize,
        contract: Address,
        account: Address,
        amount: alloy::primitives::U256,
        direction: WrappedOperation,
    },
}

pub(crate) fn derive_verified_changes(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    wrapped_native_token: Address,
    scope: &simulation_core::analysis::AnalysisScope<'_>,
) -> Result<Vec<VerifiedChange>, EspaceAnalysisError> {
    let sequence = collect_token_events(execution, wrapped_native_token, scope)?;
    let events = sequence.events;
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let mut final_state_expectations = HashMap::new();
    let mut wrapped_pair_proofs = HashMap::new();
    let mut verified_events = Vec::with_capacity(events.len());
    for (event_index, event) in events.iter().enumerate() {
        verified_events.push(verify_event(
            event_index,
            event,
            execution,
            &sequence.pairs,
            &mut wrapped_pair_proofs,
            &mut final_state_expectations,
        )?);
    }
    for (pair_index, pair) in sequence.pairs.iter().enumerate() {
        if !wrapped_pair_proofs
            .get(&pair_index)
            .is_some_and(|evidence| evidence.proves_single_transition(pair.amount))
        {
            return Err(state_mismatch_at(
                pair.position,
                "wrapped-native Transfer and Deposit/Withdrawal did not prove one operation",
            ));
        }
    }
    verify_final_state(&final_state_expectations, state)?;

    let mut changes = Vec::with_capacity(events.len());
    for (index, (event, verified)) in events.into_iter().zip(verified_events).enumerate() {
        match (event, verified) {
            (
                ObservedTokenEvent::Standard { checkpoint, .. },
                VerifiedTokenChange::Standard(verified),
            ) => {
                if sequence
                    .pairs
                    .iter()
                    .any(|pair| pair.transfer_event_index == index)
                {
                    continue;
                }
                changes.push(VerifiedChange::Standard {
                    position: checkpoint.position().index(),
                    change: verified.into_change(),
                });
            }
            (
                ObservedTokenEvent::Wrapped {
                    checkpoint,
                    contract,
                    account,
                    amount,
                    direction,
                },
                VerifiedTokenChange::Wrapped,
            ) => {
                changes.push(VerifiedChange::Wrapped {
                    position: checkpoint.position().index(),
                    contract,
                    account,
                    amount,
                    direction,
                });
            }
            _ => unreachable!("verification preserves the observed token event kind"),
        }
    }
    Ok(changes)
}

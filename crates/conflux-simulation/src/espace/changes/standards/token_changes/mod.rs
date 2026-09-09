mod call_evidence;
mod error;
mod event_verification;
mod events;
mod sequence_verification;
mod state_queries;
mod verified_changes;

use std::collections::HashMap;

use alloy::primitives::Address;

use crate::espace::{EspaceChangesError, EspaceExecutedTransaction, EspaceStateAccess};

use super::{super::EspaceStandardChange, load_metadata};

use self::{
    error::state_mismatch_at,
    events::{ObservedTokenEvent, collect_token_events, required_metadata_calls},
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
        metadata: contract_standards::Erc20Metadata,
    },
}

pub(crate) fn derive_verified_changes(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    wrapped_native_token: Address,
) -> Result<Vec<VerifiedChange>, EspaceChangesError> {
    let sequence = collect_token_events(execution, wrapped_native_token)?;
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
            state,
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

    let metadata_values = load_metadata(state.finalized(), required_metadata_calls(&events))?;
    let mut changes = Vec::with_capacity(events.len());
    for (event, verified) in events.into_iter().zip(verified_events) {
        match (event, verified) {
            (
                ObservedTokenEvent::Standard { occurrence, .. },
                VerifiedTokenChange::Standard(verified),
            ) => changes.push(VerifiedChange::Standard {
                position: occurrence.position().index(),
                change: verified.into_change(&metadata_values),
            }),
            (
                ObservedTokenEvent::Wrapped {
                    occurrence,
                    contract,
                    account,
                    amount,
                    direction,
                },
                VerifiedTokenChange::Wrapped,
            ) => {
                let token_metadata = metadata_values.erc20(&contract);
                changes.push(VerifiedChange::Wrapped {
                    position: occurrence.position().index(),
                    contract,
                    account,
                    amount,
                    direction,
                    metadata: token_metadata,
                });
            }
            _ => unreachable!("verification preserves the observed token event kind"),
        }
    }
    Ok(changes)
}

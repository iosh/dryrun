mod call_evidence;
mod error;
mod event_verification;
mod events;
mod sequence_verification;
mod state_queries;

use std::collections::HashMap;

use alloy::primitives::Address;

use crate::espace::{EspaceChangesError, EspaceExecutedTransaction, EspaceStateAccess};

use super::{
    super::{ChangeOccurrence, EspaceChange},
    load_metadata,
};

use self::{
    error::state_mismatch_at,
    events::{ObservedTokenEvent, WrappedOperation, collect_token_events, required_metadata_calls},
    sequence_verification::{verify_event, verify_final_state},
};

pub(crate) fn derive_changes(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    wrapped_native_token: Address,
) -> Result<Vec<ChangeOccurrence>, EspaceChangesError> {
    let sequence = collect_token_events(execution, wrapped_native_token)?;
    let events = sequence.events;
    if events.is_empty() {
        return Ok(Vec::new());
    }

    let mut final_state_expectations = HashMap::new();
    let mut wrapped_pair_proofs = HashMap::new();
    for (event_index, event) in events.iter().enumerate() {
        verify_event(
            event_index,
            event,
            execution,
            state,
            &sequence.pairs,
            &mut wrapped_pair_proofs,
            &mut final_state_expectations,
        )?;
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
    for event in events {
        match event {
            ObservedTokenEvent::Standard {
                occurrence,
                decoded,
            } => changes.push(ChangeOccurrence::new(
                occurrence.position().index(),
                EspaceChange::Standard(metadata_values.standard_change(decoded)),
            )),
            ObservedTokenEvent::Wrapped {
                occurrence,
                contract,
                account,
                amount,
                direction,
            } => {
                let token_metadata = metadata_values.erc20(&contract);
                let change = match direction {
                    WrappedOperation::Deposit => EspaceChange::WrappedNativeDeposit {
                        contract_address: contract,
                        account,
                        raw_amount: amount,
                        metadata: token_metadata,
                    },
                    WrappedOperation::Withdrawal => EspaceChange::WrappedNativeWithdrawal {
                        contract_address: contract,
                        account,
                        raw_amount: amount,
                        metadata: token_metadata,
                    },
                };
                changes.push(ChangeOccurrence::new(occurrence.position().index(), change));
            }
        }
    }
    Ok(changes)
}

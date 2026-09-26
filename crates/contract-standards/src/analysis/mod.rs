mod call_evidence;
pub mod dai;
mod error;
mod event_verification;
mod events;
mod sequence_verification;
mod state_queries;
mod support;
pub mod view;
pub mod weth9;

use crate::StandardChange;
use alloy_primitives::{Address, B256, U256, keccak256};
use std::{collections::HashMap, sync::LazyLock};

pub use error::AnalysisError;
use events::ObservedTokenEvent;
pub use events::WrappedOperation;
use sequence_verification::VerifiedTokenEvent;
pub use support::ReviewedStandardImplementation;
use view::TokenView;

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug)]
pub enum VerifiedChange {
    Standard {
        position: usize,
        change: StandardChange<Address>,
    },
    Wrapped {
        position: usize,
        contract: Address,
        account: Address,
        amount: U256,
        direction: WrappedOperation,
    },
}

/// Verifies event sequences against calls, checkpoint states and the final state.
/// The caller must separately establish implementation support and fact coverage.
pub fn analyze(
    view: &dyn TokenView,
    wrapped_native_token: Option<Address>,
) -> Result<Vec<VerifiedChange>, AnalysisError> {
    let sequence = events::collect_token_events(view, wrapped_native_token)?;
    let events = sequence.events;
    let mut expectations = HashMap::new();
    let mut pair_proofs = HashMap::new();
    let mut verified = Vec::with_capacity(events.len());
    for (index, event) in events.iter().enumerate() {
        verified.push(sequence_verification::verify_event(
            index,
            event,
            view,
            &sequence.pairs,
            &mut pair_proofs,
            &mut expectations,
        )?);
    }
    for (index, pair) in sequence.pairs.iter().enumerate() {
        if !pair_proofs
            .get(&index)
            .is_some_and(|proof| proof.proves_single_transition(pair.amount))
        {
            return Err(error::validation_error_at(
                pair.position,
                "wrapped Transfer and Deposit/Withdrawal do not prove one transition",
            ));
        }
    }
    sequence_verification::verify_final_state(&expectations, view)?;
    let mut changes = Vec::with_capacity(events.len());
    for (index, (event, verified)) in events.into_iter().zip(verified).enumerate() {
        match (event, verified) {
            (
                ObservedTokenEvent::Standard { checkpoint, .. },
                VerifiedTokenEvent::Standard { primary, implicit },
            ) => {
                for change in implicit {
                    changes.push(VerifiedChange::Standard {
                        position: checkpoint.position(),
                        change,
                    });
                }
                // A paired wrapped operation owns this single token movement.
                if !sequence
                    .pairs
                    .iter()
                    .any(|pair| pair.transfer_event_index == index)
                {
                    changes.push(VerifiedChange::Standard {
                        position: checkpoint.position(),
                        change: primary,
                    });
                }
            }
            (
                ObservedTokenEvent::Wrapped {
                    checkpoint,
                    contract,
                    account,
                    amount,
                    direction,
                },
                VerifiedTokenEvent::Wrapped,
            ) => {
                changes.push(VerifiedChange::Wrapped {
                    position: checkpoint.position(),
                    contract,
                    account,
                    amount,
                    direction,
                });
            }
            _ => unreachable!("event verification preserves the candidate variant"),
        }
    }
    Ok(changes)
}

use crate::EvmAnalysisView;
use simulation_core::observation::LogFilter;
mod call_evidence;
mod error;
mod event_verification;
mod events;
mod metadata;
mod sequence_verification;
mod state_queries;
mod verified_changes;

use std::{collections::HashMap, sync::LazyLock};

use alloy::primitives::{Address, B256, keccak256};

use crate::{
    EvmAnalysisError, EvmChangeRules, EvmChangeSet, EvmChangeSetBuilder,
    changeset::{EvmWrappedNativeDepositChange, EvmWrappedNativeWithdrawalChange},
    execution::EvmTransactionExecution,
    state::EvmStateAccess,
};

use self::{
    error::state_mismatch_at,
    events::{ObservedTokenEvent, WrappedOperation, collect_token_events},
    metadata::load_metadata,
    sequence_verification::{verify_event, verify_final_state},
    verified_changes::VerifiedTokenEvent,
};

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug, Clone, Copy)]
pub(crate) struct EvmTokenChangeRules {
    wrapped_native_token: Option<Address>,
}

impl EvmTokenChangeRules {
    pub(crate) const fn new(wrapped_native_token: Option<Address>) -> Self {
        Self {
            wrapped_native_token,
        }
    }

    fn derive_verified_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
    ) -> Result<EvmChangeSet, EvmAnalysisError> {
        let sequence = collect_token_events(execution, self.wrapped_native_token)?;
        let events = sequence.events;
        if events.is_empty() {
            return Ok(EvmChangeSet::default());
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

        let metadata_values = load_metadata(&events, state);
        let mut builder = EvmChangeSetBuilder::new();
        for (event, verified_event) in events.into_iter().zip(verified_events) {
            match (event, verified_event) {
                (
                    ObservedTokenEvent::Standard { checkpoint, .. },
                    VerifiedTokenEvent::Standard(change),
                ) => {
                    let change = change.into_change(&metadata_values);
                    builder.standard(checkpoint.position(), change)?;
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
                    let token_metadata = metadata_values.erc20(&contract);
                    match direction {
                        WrappedOperation::Deposit => builder.wrapped_native_deposit(
                            checkpoint.position(),
                            EvmWrappedNativeDepositChange {
                                contract_address: contract,
                                account,
                                raw_amount: amount,
                                metadata: token_metadata,
                            },
                        )?,
                        WrappedOperation::Withdrawal => builder.wrapped_native_withdrawal(
                            checkpoint.position(),
                            EvmWrappedNativeWithdrawalChange {
                                contract_address: contract,
                                account,
                                raw_amount: amount,
                                metadata: token_metadata,
                            },
                        )?,
                    }
                }
                _ => unreachable!("verification preserves the observed token event kind"),
            }
        }

        Ok(builder.finish())
    }
}

impl EvmChangeRules for EvmTokenChangeRules {
    fn checkpoint_filters(&self) -> Vec<LogFilter> {
        let mut filters = Vec::new();
        for topic0 in contract_standards::supported_event_topics() {
            filters.push(LogFilter {
                address: None,
                topic0: *topic0,
            });
        }
        if let Some(address) = self.wrapped_native_token {
            filters.push(LogFilter {
                address: Some(address),
                topic0: *DEPOSIT_TOPIC0,
            });
            filters.push(LogFilter {
                address: Some(address),
                topic0: *WITHDRAWAL_TOPIC0,
            });
        }
        filters
    }

    fn derive_changes(&self, view: EvmAnalysisView<'_>) -> Result<EvmChangeSet, EvmAnalysisError> {
        let execution = view.execution();
        let state = view.state();
        self.derive_verified_changes(execution, state)
    }
}

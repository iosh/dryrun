use crate::EvmAnalysisView;
use simulation_core::observation::LogFilter;
use simulation_core::{analysis::*, changes::ChangePosition};
mod call_evidence;
mod error;
mod event_verification;
mod events;
mod sequence_verification;
mod state_queries;
mod verified_changes;

use std::{collections::HashMap, sync::LazyLock};

use alloy::primitives::{Address, B256, keccak256};

use crate::{
    EvmAnalysisDomain, EvmAnalysisError, EvmChangeSet, EvmStateChange,
    changeset::EvmWrappedNativeDepositChange, execution::EvmTransactionExecution,
    state::EvmStateAccess,
};

use self::{
    error::state_mismatch_at,
    events::{ObservedTokenEvent, WrappedOperation, collect_token_events},
    sequence_verification::{verify_event, verify_final_state},
    verified_changes::VerifiedTokenEvent,
};

static DEPOSIT_TOPIC0: LazyLock<B256> = LazyLock::new(|| keccak256("Deposit(address,uint256)"));
static WITHDRAWAL_TOPIC0: LazyLock<B256> =
    LazyLock::new(|| keccak256("Withdrawal(address,uint256)"));

#[derive(Debug, Clone, Copy)]
pub(crate) struct TokenAnalyzer {
    wrapped_native_token: Option<Address>,
}

impl TokenAnalyzer {
    pub(crate) const fn new(wrapped_native_token: Option<Address>) -> Self {
        Self {
            wrapped_native_token,
        }
    }

    fn derive_verified_changes(
        &self,
        execution: &EvmTransactionExecution,
        state: &EvmStateAccess,
        scope: &AnalysisScope<'_>,
    ) -> Result<EvmChangeSet, EvmAnalysisError> {
        let sequence = collect_token_events(execution, self.wrapped_native_token, scope)?;
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

        let mut changes = EvmChangeSet::new();
        for (index, (event, verified_event)) in events.into_iter().zip(verified_events).enumerate()
        {
            match (event, verified_event) {
                (
                    ObservedTokenEvent::Standard { checkpoint, .. },
                    VerifiedTokenEvent::Standard(change),
                ) => {
                    if sequence
                        .pairs
                        .iter()
                        .any(|pair| pair.transfer_event_index == index)
                    {
                        continue;
                    }
                    changes.insert(
                        ChangePosition::Execution(checkpoint.position().index()),
                        EvmStateChange::Standard(change.into_change()),
                    );
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
                    let change = EvmWrappedNativeDepositChange {
                        contract_address: contract,
                        account,
                        raw_amount: amount,
                    };
                    let change = match direction {
                        WrappedOperation::Deposit => EvmStateChange::WrappedNativeDeposit(change),
                        WrappedOperation::Withdrawal => {
                            EvmStateChange::WrappedNativeWithdrawal(change)
                        }
                    };
                    changes.insert(
                        ChangePosition::Execution(checkpoint.position().index()),
                        change,
                    );
                }
                _ => unreachable!("verification preserves the observed token event"),
            }
        }
        Ok(changes)
    }
}

impl Analyzer<EvmAnalysisDomain> for TokenAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "ethereum-contracts",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        _: EvmAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, EvmAnalysisError> {
        Ok(scope.select(|fact| {
            matches!(
                fact.kind,
                FactKind::Call
                    | FactKind::Log
                    | FactKind::StorageWrite
                    | FactKind::Create
                    | FactKind::Destroy
            )
        }))
    }

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

    fn analyze<'a>(
        &self,
        view: EvmAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, EvmChangeSet>, EvmAnalysisError> {
        check_contract_support(view, scope)?;
        let changes = self.derive_verified_changes(view.execution(), view.state(), scope)?;
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::NoRelevantEffects))
    }
}

fn check_contract_support(
    view: EvmAnalysisView<'_>,
    scope: &AnalysisScope<'_>,
) -> Result<(), EvmAnalysisError> {
    if let Some(fact) = scope.facts().find(|fact| fact.kind != FactKind::Call) {
        return Err(EvmAnalysisError::Unsupported {
            details: format!(
                "no reviewed contract implementation for {:?} at {:?}",
                fact.kind, fact.position
            ),
        });
    }
    let frames: std::collections::BTreeSet<_> =
        scope.facts().filter_map(|fact| fact.frame_id).collect();
    for frame in view.execution().committed_frames() {
        if !frames.contains(&frame.id().index()) {
            continue;
        }
        let crate::EvmFrameAction::Call {
            bytecode_address, ..
        } = frame.action()
        else {
            continue;
        };
        if !view.state().initial().code(*bytecode_address)?.is_empty()
            || !view.state().finalized().code(*bytecode_address)?.is_empty()
        {
            return Err(EvmAnalysisError::Unsupported {
                details: format!("no reviewed implementation for code at {bytecode_address}"),
            });
        }
    }
    Ok(())
}

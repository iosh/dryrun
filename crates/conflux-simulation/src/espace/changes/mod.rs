mod standards;
mod wrapped_native;
use super::EspaceAnalysisError;
use super::{EspaceAnalysisDomain, EspaceAnalysisView, EspaceFrameAction};
use simulation_core::analysis::*;
pub(crate) mod native;
use super::{EspaceAccountState, EspaceExecutedTransaction, EspaceStateAccess};
use alloy_primitives::Address;
use simulation_core::changes::ChangePosition;
use std::collections::BTreeMap;

pub type EspaceChange = simulation_core::changes::AssetChange;
pub type EspaceStateChange = simulation_core::changes::AssetChange;
pub type EspaceChangeSet = simulation_core::changes::AssetChangeSet;
pub type EspaceNativeCurrency = simulation_core::changes::NativeCurrency;
pub type EspaceNativeTransferChange = simulation_core::changes::NativeTransfer;
pub type EspaceSelfDestructBurnChange = simulation_core::changes::NativeBurn;
pub type EspaceAccountDelegationChange = simulation_core::changes::DelegationChange;
pub type EspaceAccountDelegation = simulation_core::changes::AccountDelegation;
pub type EspaceWrappedNativeDepositChange = simulation_core::changes::WrappedNativeChange;
pub type EspaceWrappedNativeWithdrawalChange = simulation_core::changes::WrappedNativeChange;
pub type EspaceStandardChange = contract_standards::StandardChange<Address>;
pub type EspaceChanges = simulation_core::simulation::Changes<EspaceChangeSet, EspaceAnalysisError>;

pub(crate) fn derive_delegation(
    execution: &EspaceExecutedTransaction,
    state: &EspaceStateAccess,
    scope: &AnalysisScope<'_>,
) -> Result<EspaceChangeSet, EspaceAnalysisError> {
    let accounts: std::collections::BTreeSet<_> = scope
        .facts()
        .filter(|fact| fact.kind == FactKind::Delegation)
        .filter_map(|fact| fact.address)
        .collect();
    let mut authorizations = BTreeMap::<Address, Vec<_>>::new();
    for authorization in execution.applied_authorizations() {
        if !accounts.contains(&authorization.account()) {
            continue;
        }
        authorizations
            .entry(authorization.account())
            .or_default()
            .push(authorization);
    }

    let mut changes = EspaceChangeSet::new();
    for (account, authorizations) in authorizations {
        let before_account = state.initial().read_account(account)?;
        let after_account = state.finalized().read_account(account)?;
        let before = delegation_state(account, &before_account)?;
        let after = delegation_state(account, &after_account)?;

        // The executor increments the transaction sender nonce before it
        // processes the EIP-7702 authorization list.  That increment is
        // part of the authority's nonce when the sender authorizes itself.
        let mut expected_nonce = before.nonce;
        if account == execution.transaction_sender() {
            expected_nonce =
                expected_nonce
                    .checked_add(1)
                    .ok_or_else(|| EspaceAnalysisError::Validation {
                        details: format!(
                            "transaction sender nonce overflow for delegation account {account}"
                        ),
                    })?;
        }
        for authorization in &authorizations {
            if authorization.nonce() != expected_nonce {
                return Err(EspaceAnalysisError::Validation {
                    details: format!(
                        "successful authorization for {account} used nonce {}, expected {expected_nonce}",
                        authorization.nonce()
                    ),
                });
            }
            expected_nonce =
                expected_nonce
                    .checked_add(1)
                    .ok_or_else(|| EspaceAnalysisError::Validation {
                        details: format!(
                            "successful authorization nonce overflow for account {account}"
                        ),
                    })?;
        }

        let expected_delegate = authorizations.last().and_then(|authorization| {
            (authorization.delegate() != Address::ZERO).then_some(authorization.delegate())
        });
        if after.nonce != expected_nonce || after.delegate != expected_delegate {
            return Err(EspaceAnalysisError::Validation {
                details: format!(
                    "final delegation state for {account} does not match successful authorization results"
                ),
            });
        }
        changes.insert(
            ChangePosition::BeforeExecution,
            EspaceStateChange::AccountDelegation(EspaceAccountDelegationChange {
                account,
                before,
                after,
            }),
        );
    }
    Ok(changes)
}

fn delegation_state(
    account: Address,
    state: &EspaceAccountState,
) -> Result<EspaceAccountDelegation, EspaceAnalysisError> {
    let nonce = u64::try_from(state.nonce()).map_err(|_| EspaceAnalysisError::Validation {
        details: format!("eSpace nonce for delegation account {account} exceeds u64"),
    })?;
    Ok(EspaceAccountDelegation {
        delegate: state.delegation(),
        nonce,
    })
}

pub(crate) struct TokenAnalyzer {
    wrapped_native: Address,
}
impl TokenAnalyzer {
    pub(crate) fn new(wrapped_native: Address) -> Self {
        Self { wrapped_native }
    }
    pub(crate) const MAINNET_WCFX: Address =
        alloy_primitives::address!("14b2d3bc65e74dae1030eafd8ac30c533c976a9b");
}
impl Analyzer<EspaceAnalysisDomain> for TokenAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "espace-contracts",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn checkpoint_filters(&self) -> Vec<simulation_core::observation::LogFilter> {
        let mut filters: Vec<_> = contract_standards::supported_event_topics()
            .iter()
            .map(|&topic0| simulation_core::observation::LogFilter {
                address: None,
                topic0,
            })
            .collect();
        filters.extend(wrapped_native::checkpoint_filters(self.wrapped_native));
        filters
    }
    fn select<'a>(
        &self,
        _: EspaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, EspaceAnalysisError> {
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
    fn analyze<'a>(
        &self,
        view: EspaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, EspaceChangeSet>, EspaceAnalysisError> {
        check_contract_support(view, scope)?;
        let mut changes = EspaceChangeSet::new();
        for change in standards::derive_verified_changes(
            view.execution(),
            view.state(),
            self.wrapped_native,
            scope,
        )? {
            let (position, change) = match change {
                standards::VerifiedChange::Standard { position, change } => {
                    (position, EspaceChange::Standard(change))
                }
                standards::VerifiedChange::Wrapped {
                    position,
                    contract,
                    account,
                    amount,
                    direction,
                } => {
                    let change = EspaceWrappedNativeDepositChange {
                        contract_address: contract,
                        account,
                        raw_amount: amount,
                    };
                    (
                        position,
                        match direction {
                            standards::WrappedOperation::Deposit => {
                                EspaceChange::WrappedNativeDeposit(change)
                            }
                            standards::WrappedOperation::Withdrawal => {
                                EspaceChange::WrappedNativeWithdrawal(change)
                            }
                        },
                    )
                }
            };
            changes.insert(ChangePosition::Execution(position), change);
        }
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::NoRelevantEffects))
    }
}
fn check_contract_support(
    view: EspaceAnalysisView<'_>,
    scope: &AnalysisScope<'_>,
) -> Result<(), EspaceAnalysisError> {
    if let Some(fact) = scope
        .facts()
        .find(|fact| fact.kind != FactKind::Call || fact.chain.space != ExecutionSpace::Espace)
    {
        return Err(EspaceAnalysisError::Unsupported {
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
        let EspaceFrameAction::Call { code_address, .. } = frame.action() else {
            continue;
        };
        for reader in [view.state().initial(), view.state().finalized()] {
            if reader
                .read_account(*code_address)?
                .code()
                .is_some_and(|code| !code.is_empty())
            {
                return Err(EspaceAnalysisError::Unsupported {
                    details: format!("no reviewed implementation for code at {code_address}"),
                });
            }
        }
    }
    Ok(())
}

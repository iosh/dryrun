use std::{collections::BTreeSet, sync::Arc};

use contract_standards::analysis::{AnalysisError, VerifiedChange, view::TokenView};
use simulation_core::{
    analysis::*,
    changes::{
        AccountDelegation, AssetChange, ChangePosition, DelegationChange, NativeBurn,
        NativeTransfer,
    },
    contract_analysis::{ContractAnalysisDomain, default_contract_analyzers},
};

use crate::{
    EvmAnalysisError, EvmBlockContext, EvmChangeSet, EvmFrameAction, EvmNativeCurrency,
    EvmStateAccess, EvmTransactionExecution, TypedTransaction, execution::NativeMovement,
    token_view::EvmTokenView,
};

#[derive(Clone, Copy)]
pub struct EvmAnalysisView<'a> {
    pub(crate) context: &'a EvmBlockContext,
    pub(crate) transaction: &'a TypedTransaction,
    pub(crate) execution: &'a EvmTransactionExecution,
}

impl<'a> EvmAnalysisView<'a> {
    pub fn context(self) -> &'a EvmBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a TypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a EvmTransactionExecution {
        self.execution
    }
    pub fn state(self) -> &'a EvmStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = super::EvmLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }

    pub(crate) fn facts(self) -> Vec<ExecutionFact> {
        let chain = ChainScope {
            chain_id: self.transaction.common().chain_id,
            space: ExecutionSpace::Evm,
        };
        let mut facts = Vec::new();
        let mut push = |position, kind, address, frame_id| {
            facts.push(ExecutionFact {
                chain,
                height: self.context.number,
                position,
                kind,
                address,
                frame_id,
            })
        };
        for frame in self.execution.committed_frames() {
            let (kind, address) = match frame.action() {
                EvmFrameAction::Call { target, .. } => (FactKind::Call, Some(*target)),
                EvmFrameAction::Create {
                    created_address, ..
                } => (FactKind::Create, *created_address),
            };
            push(
                ChangePosition::Execution(frame.position().index()),
                kind,
                address,
                Some(frame.id().index()),
            );
        }
        for log in self.execution.committed_logs() {
            push(
                ChangePosition::Execution(log.position().index()),
                FactKind::Log,
                Some(log.log().address),
                Some(log.frame_id().index()),
            );
        }
        for write in self.execution.storage_writes() {
            push(
                ChangePosition::Execution(write.position().index()),
                FactKind::StorageWrite,
                Some(write.address()),
                Some(write.frame_id().index()),
            );
        }
        for effect in self.execution.committed_selfdestructs() {
            if effect.destroys_contract() {
                push(
                    ChangePosition::Execution(effect.position().index()),
                    FactKind::Destroy,
                    Some(effect.contract()),
                    Some(effect.frame_id().index()),
                );
            }
        }
        for movement in self.execution.native_movements() {
            let position = match movement {
                NativeMovement::Transfer { position, .. }
                | NativeMovement::SelfDestructBurn { position, .. } => position,
            };
            push(
                ChangePosition::Execution(position.index()),
                FactKind::NativeMovement,
                None,
                None,
            );
        }
        for &account in self.execution.applied_authorization_accounts() {
            push(
                ChangePosition::BeforeExecution,
                FactKind::Delegation,
                Some(account),
                None,
            );
        }
        facts
    }
}

pub enum EvmAnalysisDomain {}
pub type EvmAnalyzerRegistry = AnalyzerRegistry<EvmAnalysisDomain>;

impl AnalysisDomain for EvmAnalysisDomain {
    type View<'a> = EvmAnalysisView<'a>;
    type Changes = EvmChangeSet;
    type Error = EvmAnalysisError;

    fn facts(view: Self::View<'_>) -> Vec<ExecutionFact> {
        view.facts()
    }
}

impl ContractAnalysisDomain for EvmAnalysisDomain {
    fn token_view<'a>(
        view: Self::View<'a>,
        _: ChainScope,
    ) -> Result<Box<dyn TokenView + 'a>, Self::Error> {
        Ok(Box::new(EvmTokenView {
            execution: view.execution,
        }))
    }
    fn insert_contract_change(
        _: Self::View<'_>,
        changes: &mut EvmChangeSet,
        _: ChainScope,
        change: VerifiedChange,
    ) -> Result<(), EvmAnalysisError> {
        changes.insert_verified(change);
        Ok(())
    }
    fn contract_error(error: AnalysisError) -> EvmAnalysisError {
        error.into()
    }
}

pub(crate) fn default_registry(currency: EvmNativeCurrency) -> EvmAnalyzerRegistry {
    let mut rules = default_contract_analyzers::<EvmAnalysisDomain>();
    rules.push(Arc::new(NativeAnalyzer { currency }));
    rules.push(Arc::new(DelegationAnalyzer));
    EvmAnalyzerRegistry::new(rules)
        .expect("built-in Ethereum analyzer IDs and deployments are valid")
}

struct NativeAnalyzer {
    currency: EvmNativeCurrency,
}

impl Analyzer<EvmAnalysisDomain> for NativeAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "ethereum-native",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        _: EvmAnalysisView<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, EvmAnalysisError> {
        Ok(candidates.select(|fact| fact.kind == FactKind::NativeMovement))
    }
    fn analyze<'a>(
        &self,
        view: EvmAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, EvmChangeSet>, EvmAnalysisError> {
        let positions: BTreeSet<_> = scope.facts().map(|fact| fact.position).collect();
        let mut changes = EvmChangeSet::new();
        for movement in view.execution.native_movements() {
            let (position, change) = match *movement {
                NativeMovement::Transfer {
                    position,
                    from,
                    to,
                    amount,
                } => (
                    position,
                    AssetChange::NativeTransfer(NativeTransfer {
                        from,
                        to,
                        raw_amount: amount,
                        currency: self.currency.clone(),
                    }),
                ),
                NativeMovement::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                } => (
                    position,
                    AssetChange::SelfDestructBurn(NativeBurn {
                        contract_address: contract,
                        raw_amount: amount,
                        currency: self.currency.clone(),
                    }),
                ),
            };
            if positions.contains(&ChangePosition::Execution(position.index())) {
                changes.insert(ChangePosition::Execution(position.index()), change);
            }
        }
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::Protocol))
    }
}

struct DelegationAnalyzer;
impl Analyzer<EvmAnalysisDomain> for DelegationAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "ethereum-delegation",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        _: EvmAnalysisView<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, EvmAnalysisError> {
        Ok(candidates.select(|fact| fact.kind == FactKind::Delegation))
    }
    fn analyze<'a>(
        &self,
        view: EvmAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, EvmChangeSet>, EvmAnalysisError> {
        let mut changes = EvmChangeSet::new();
        for fact in scope.facts() {
            let account = fact.address.expect("delegation facts have an account");
            let before = view.state().initial().read_account(account)?;
            let after = view.state().finalized().read_account(account)?;
            changes.insert(
                ChangePosition::BeforeExecution,
                AssetChange::AccountDelegation(DelegationChange {
                    account,
                    before: AccountDelegation {
                        delegate: before.delegation(),
                        nonce: before.nonce(),
                    },
                    after: AccountDelegation {
                        delegate: after.delegation(),
                        nonce: after.nonce(),
                    },
                }),
            );
        }
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::Protocol))
    }
}

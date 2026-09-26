use std::sync::Arc;

use contract_standards::analysis::{AnalysisError, VerifiedChange, view::TokenView};
use simulation_core::{
    analysis::*,
    changes::{AssetChange, AssetChangeSet, ChangePosition},
    contract_analysis::{ContractAnalysisDomain, default_contract_analyzers},
};

use super::{
    EspaceAnalysisError, EspaceBlockContext, EspaceCallKind, EspaceChangeSet,
    EspaceExecutedTransaction, EspaceExecutionSpace, EspaceFrameAction, EspaceNativeCurrency,
    EspaceStateAccess, EspaceTypedTransaction, changes, token_view::EspaceTokenView,
};

#[derive(Clone, Copy)]
pub struct EspaceAnalysisView<'a> {
    pub(crate) context: &'a EspaceBlockContext,
    pub(crate) transaction: &'a EspaceTypedTransaction,
    pub(crate) execution: &'a EspaceExecutedTransaction,
    pub(crate) core_chain_id: u32,
}

impl<'a> EspaceAnalysisView<'a> {
    pub fn context(self) -> &'a EspaceBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a EspaceTypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a EspaceExecutedTransaction {
        self.execution
    }
    pub fn state(self) -> &'a EspaceStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = super::EspaceLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }

    fn chain(self, space: EspaceExecutionSpace) -> ChainScope {
        match space {
            EspaceExecutionSpace::Espace => ChainScope {
                chain_id: self.transaction.common().chain_id,
                space: ExecutionSpace::Espace,
            },
            EspaceExecutionSpace::Core => ChainScope {
                chain_id: u64::from(self.core_chain_id),
                space: ExecutionSpace::Core,
            },
        }
    }
    pub(crate) fn facts(self) -> Vec<ExecutionFact> {
        let mut facts = Vec::new();
        let mut push = |space, position, kind, address, frame_id| {
            facts.push(ExecutionFact {
                chain: self.chain(space),
                height: self.context.number,
                position,
                kind,
                address,
                frame_id,
            })
        };
        for frame in self.execution.committed_frames() {
            let (kind, address, value) = match frame.action() {
                EspaceFrameAction::Call {
                    target,
                    kind,
                    value,
                    ..
                } => (
                    FactKind::Call,
                    *target,
                    if *kind == EspaceCallKind::Call {
                        *value
                    } else {
                        alloy_primitives::U256::ZERO
                    },
                ),
                EspaceFrameAction::Create {
                    actual_address,
                    value,
                    ..
                } => (FactKind::Create, *actual_address, *value),
            };
            let position = ChangePosition::Execution(frame.position().index());
            push(
                frame.space(),
                position,
                kind,
                Some(address),
                Some(frame.id().index()),
            );
            if !value.is_zero() {
                push(
                    frame.space(),
                    position,
                    FactKind::NativeMovement,
                    None,
                    Some(frame.id().index()),
                );
            }
        }
        for log in self.execution.committed_logs() {
            push(
                log.space(),
                ChangePosition::Execution(log.position().index()),
                FactKind::Log,
                Some(log.address()),
                Some(log.frame_id().index()),
            );
        }
        for write in self.execution.storage_writes() {
            push(
                write.space(),
                ChangePosition::Execution(write.position().index()),
                FactKind::StorageWrite,
                Some(write.address()),
                Some(write.frame_id().index()),
            );
        }
        for transfer in self.execution.internal_transfers() {
            push(
                transfer.space(),
                ChangePosition::Execution(transfer.position().index()),
                FactKind::NativeMovement,
                None,
                transfer.frame_id().map(|id| id.index()),
            );
        }
        for (position, address) in &self.execution.removed_contracts {
            let space = match address.space {
                cfx_types::Space::Native => EspaceExecutionSpace::Core,
                cfx_types::Space::Ethereum => EspaceExecutionSpace::Espace,
            };
            push(
                space,
                ChangePosition::Execution(*position),
                FactKind::Destroy,
                Some(crate::primitive::address_from_cfx(address.address)),
                None,
            );
        }
        for authorization in self.execution.applied_authorizations() {
            push(
                EspaceExecutionSpace::Espace,
                ChangePosition::BeforeExecution,
                FactKind::Delegation,
                Some(authorization.account()),
                None,
            );
        }
        facts
    }
}

pub enum EspaceAnalysisDomain {}
pub type EspaceAnalyzerRegistry = AnalyzerRegistry<EspaceAnalysisDomain>;

impl AnalysisDomain for EspaceAnalysisDomain {
    type View<'a> = EspaceAnalysisView<'a>;
    type Changes = EspaceChangeSet;
    type Error = EspaceAnalysisError;

    fn facts(view: Self::View<'_>) -> Vec<ExecutionFact> {
        view.facts()
    }
}
impl ContractAnalysisDomain for EspaceAnalysisDomain {
    fn token_view<'a>(
        view: Self::View<'a>,
        chain: ChainScope,
    ) -> Result<Box<dyn TokenView + 'a>, Self::Error> {
        if chain.space != ExecutionSpace::Espace {
            return Err(AnalysisError::unsupported(
                "Core Space contract calls from an eSpace transaction are not supported",
            )
            .into());
        }
        Ok(Box::new(EspaceTokenView {
            execution: view.execution,
        }))
    }
    fn insert_contract_change(
        _: Self::View<'_>,
        changes: &mut EspaceChangeSet,
        _: ChainScope,
        change: VerifiedChange,
    ) -> Result<(), EspaceAnalysisError> {
        changes.insert_verified(change);
        Ok(())
    }
    fn contract_error(error: AnalysisError) -> EspaceAnalysisError {
        error.into()
    }
}

pub(crate) fn default_registry(currency: EspaceNativeCurrency) -> EspaceAnalyzerRegistry {
    let mut rules = default_contract_analyzers::<EspaceAnalysisDomain>();
    rules.push(Arc::new(ProtocolAnalyzer { currency }));
    EspaceAnalyzerRegistry::new(rules)
        .expect("built-in eSpace analyzer IDs and deployments are valid")
}

struct ProtocolAnalyzer {
    currency: EspaceNativeCurrency,
}
impl Analyzer<EspaceAnalysisDomain> for ProtocolAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "espace-protocol",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        _: EspaceAnalysisView<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, EspaceAnalysisError> {
        Ok(candidates.select(|fact| {
            matches!(
                fact.kind,
                FactKind::NativeMovement | FactKind::Delegation | FactKind::Protocol
            )
        }))
    }
    fn analyze<'a>(
        &self,
        view: EspaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, EspaceChangeSet>, EspaceAnalysisError> {
        let mut changes = AssetChangeSet::new();
        for operation in changes::native::collect_native_operations(view.execution, scope)? {
            use changes::native::NativeOperation;
            let change = match operation {
                NativeOperation::AccountTransfer {
                    from, to, amount, ..
                } => AssetChange::NativeTransfer(simulation_core::changes::NativeTransfer {
                    from,
                    to,
                    raw_amount: amount,
                    currency: self.currency.clone(),
                }),
                NativeOperation::SelfDestructBurn {
                    contract, amount, ..
                } => AssetChange::SelfDestructBurn(simulation_core::changes::NativeBurn {
                    contract_address: contract,
                    raw_amount: amount,
                    currency: self.currency.clone(),
                }),
            };
            let position = ChangePosition::Execution(operation.position());
            changes.insert(position, change);
        }
        for change in changes::derive_delegation(view.execution, view.state(), scope)?.into_items()
        {
            changes.insert(ChangePosition::BeforeExecution, change);
        }
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::Protocol))
    }
}

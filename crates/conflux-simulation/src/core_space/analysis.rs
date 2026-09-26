use std::sync::Arc;

use cfx_types::Space;
use contract_standards::analysis::{AnalysisError, VerifiedChange, view::TokenView};
use simulation_core::{
    analysis::*,
    changes::{AssetChange, ChangePosition},
    contract_analysis::{ContractAnalysisDomain, default_contract_analyzers},
};

use super::{
    CoreSpaceAnalysisError, CoreSpaceBlockContext, CoreSpaceChange, CoreSpaceChangeSet,
    CoreSpaceExecutedTransaction, CoreSpaceNativeCurrency, CoreSpaceStateAccess,
    CoreSpaceTypedTransaction, changes::derive_protocol_changes, token_view::CoreTokenView,
};
use crate::{
    execution::{FrameAction, TraceEvent},
    primitive::address_from_cfx,
};

#[derive(Clone, Copy)]
pub struct CoreSpaceAnalysisView<'a> {
    pub(crate) context: &'a CoreSpaceBlockContext,
    pub(crate) transaction: &'a CoreSpaceTypedTransaction,
    pub(crate) execution: &'a CoreSpaceExecutedTransaction,
    pub(crate) espace_chain_id: u32,
}

impl<'a> CoreSpaceAnalysisView<'a> {
    pub fn context(self) -> &'a CoreSpaceBlockContext {
        self.context
    }
    pub fn transaction(self) -> &'a CoreSpaceTypedTransaction {
        self.transaction
    }
    pub fn execution(self) -> &'a CoreSpaceExecutedTransaction {
        self.execution
    }
    pub fn state(self) -> &'a CoreSpaceStateAccess {
        self.execution.state()
    }
    pub fn log_checkpoints(self) -> impl Iterator<Item = super::CoreSpaceLogCheckpoint<'a>> {
        self.execution.log_checkpoints()
    }

    fn chain(self, space: Space) -> ChainScope {
        match space {
            Space::Native => ChainScope {
                chain_id: u64::from(self.transaction.common().chain_id),
                space: ExecutionSpace::Core,
            },
            Space::Ethereum => ChainScope {
                chain_id: u64::from(self.espace_chain_id),
                space: ExecutionSpace::Espace,
            },
        }
    }

    pub(crate) fn facts(self) -> Vec<ExecutionFact> {
        let mut facts = Vec::new();
        let mut push = |space, position, kind, address, frame_id| {
            facts.push(ExecutionFact {
                chain: self.chain(space),
                height: self.context.epoch_number,
                position,
                kind,
                address,
                frame_id,
            })
        };
        for event in self.execution.trace().events() {
            match event {
                TraceEvent::ContractRemoved { position, address } => {
                    push(
                        address.space,
                        ChangePosition::Execution(*position),
                        FactKind::Destroy,
                        Some(address_from_cfx(address.address)),
                        None,
                    );
                }
                TraceEvent::FrameStart { position, frame_id } => {
                    let frame = self.execution.trace().frame(*frame_id);
                    let (kind, address, value) = match &frame.action {
                        FrameAction::Call {
                            target,
                            transferred_value,
                            ..
                        } => (FactKind::Call, *target, *transferred_value),
                        FrameAction::Create {
                            actual_created_address,
                            value,
                            ..
                        } => (
                            FactKind::Create,
                            actual_created_address.expect("committed create address is verified"),
                            *value,
                        ),
                    };
                    push(
                        frame.space,
                        ChangePosition::Execution(*position),
                        kind,
                        Some(address_from_cfx(address)),
                        Some(frame_id.index()),
                    );
                    if !value.is_zero() {
                        push(
                            frame.space,
                            ChangePosition::Execution(*position),
                            FactKind::NativeMovement,
                            None,
                            Some(frame_id.index()),
                        );
                    }
                }
                TraceEvent::Log {
                    position,
                    frame_id,
                    address,
                    ..
                } => {
                    push(
                        self.execution.trace().frame(*frame_id).space,
                        ChangePosition::Execution(*position),
                        FactKind::Log,
                        Some(address_from_cfx(*address)),
                        Some(frame_id.index()),
                    );
                }
                TraceEvent::StorageWrite {
                    position,
                    frame_id,
                    address,
                    ..
                } => {
                    push(
                        address.space,
                        ChangePosition::Execution(*position),
                        FactKind::StorageWrite,
                        Some(address_from_cfx(address.address)),
                        Some(frame_id.index()),
                    );
                }
                TraceEvent::InternalTransfer {
                    position,
                    frame_id,
                    space,
                    ..
                } => {
                    push(
                        *space,
                        ChangePosition::Execution(*position),
                        FactKind::NativeMovement,
                        None,
                        frame_id.map(|id| id.index()),
                    );
                }
            }
        }
        // Automatic collateral, sponsorship and administrative settlement has no log requirement.
        push(
            Space::Native,
            ChangePosition::Settlement,
            FactKind::Protocol,
            None,
            None,
        );
        facts
    }
}

pub enum CoreSpaceAnalysisDomain {}
pub type CoreSpaceAnalyzerRegistry = AnalyzerRegistry<CoreSpaceAnalysisDomain>;
impl AnalysisDomain for CoreSpaceAnalysisDomain {
    type View<'a> = CoreSpaceAnalysisView<'a>;
    type Changes = CoreSpaceChangeSet;
    type Error = CoreSpaceAnalysisError;

    fn facts(view: Self::View<'_>) -> Vec<ExecutionFact> {
        view.facts()
    }
}
impl ContractAnalysisDomain for CoreSpaceAnalysisDomain {
    fn token_view<'a>(
        view: Self::View<'a>,
        chain: ChainScope,
    ) -> Result<Box<dyn TokenView + 'a>, Self::Error> {
        let space = match chain.space {
            ExecutionSpace::Core => Space::Native,
            ExecutionSpace::Espace => Space::Ethereum,
            ExecutionSpace::Evm => {
                return Err(AnalysisError::unsupported(
                    "Ethereum execution facts cannot occur in a Core Space simulation",
                )
                .into());
            }
        };
        Ok(Box::new(CoreTokenView {
            execution: view.execution,
            space,
        }))
    }
    fn insert_contract_change(
        view: Self::View<'_>,
        changes: &mut CoreSpaceChangeSet,
        chain: ChainScope,
        change: VerifiedChange,
    ) -> Result<(), CoreSpaceAnalysisError> {
        let (position, change) = AssetChange::from_verified(change);
        let change = match chain.space {
            ExecutionSpace::Core => CoreSpaceChange::Asset(
                change
                    .try_map_addresses(|address| {
                        conflux_provider::CoreAddress::from_bytes(
                            address.0.0,
                            view.execution.address_network(),
                        )
                    })
                    .map_err(|error| {
                        CoreSpaceAnalysisError::rule_failure("Core Space contract address", error)
                    })?,
            ),
            ExecutionSpace::Espace => CoreSpaceChange::Espace(change),
            ExecutionSpace::Evm => {
                return Err(AnalysisError::unsupported(
                    "Ethereum changes cannot occur in a Core Space simulation",
                )
                .into());
            }
        };
        changes.insert(position, change);
        Ok(())
    }
    fn contract_error(error: AnalysisError) -> CoreSpaceAnalysisError {
        error.into()
    }
}

pub(crate) fn default_registry(
    currency: CoreSpaceNativeCurrency,
    espace_currency: crate::espace::EspaceNativeCurrency,
) -> CoreSpaceAnalyzerRegistry {
    let mut rules = default_contract_analyzers::<CoreSpaceAnalysisDomain>();
    rules.push(Arc::new(ProtocolAnalyzer {
        currency,
        espace_currency,
    }));
    CoreSpaceAnalyzerRegistry::new(rules)
        .expect("built-in Core Space analyzer IDs and deployments are valid")
}

struct ProtocolAnalyzer {
    currency: CoreSpaceNativeCurrency,
    espace_currency: crate::espace::EspaceNativeCurrency,
}
impl Analyzer<CoreSpaceAnalysisDomain> for ProtocolAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "core-space-protocol",
            layer: AnalyzerLayer::Specialized,
            priority: 100,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        view: CoreSpaceAnalysisView<'_>,
        candidates: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, CoreSpaceAnalysisError> {
        let internal_frames: std::collections::BTreeSet<_> = view
            .execution
            .trace()
            .frames()
            .filter_map(|(id, frame)| match frame.action {
                FrameAction::Call { code_address, .. }
                    if frame.space == Space::Native
                        && view.execution.is_active_internal_contract(code_address) =>
                {
                    Some(id.index())
                }
                _ => None,
            })
            .collect();
        Ok(candidates.select(|fact| {
            matches!(fact.kind, FactKind::NativeMovement | FactKind::Protocol)
                || (fact.chain.space == ExecutionSpace::Core && fact.kind == FactKind::Destroy)
                || (fact.chain.space == ExecutionSpace::Core
                    && matches!(
                        fact.kind,
                        FactKind::Call | FactKind::Log | FactKind::StorageWrite
                    )
                    && fact
                        .frame_id
                        .is_some_and(|frame| internal_frames.contains(&frame)))
        }))
    }
    fn analyze<'a>(
        &self,
        view: CoreSpaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, CoreSpaceChangeSet>, CoreSpaceAnalysisError> {
        use cfx_parameters::internal_contract_addresses::*;
        let supported = [
            ADMIN_CONTROL_CONTRACT_ADDRESS,
            SPONSOR_WHITELIST_CONTROL_CONTRACT_ADDRESS,
            STORAGE_INTEREST_STAKING_CONTRACT_ADDRESS,
            POS_REGISTER_CONTRACT_ADDRESS,
            CROSS_SPACE_CONTRACT_ADDRESS,
            PARAMS_CONTROL_CONTRACT_ADDRESS,
            CONTEXT_CONTRACT_ADDRESS,
        ];
        let frames: std::collections::BTreeSet<_> = scope
            .facts()
            .filter(|fact| fact.kind == FactKind::Call)
            .filter_map(|fact| fact.frame_id)
            .collect();
        for (id, frame) in view.execution.trace().frames() {
            if !frames.contains(&id.index()) {
                continue;
            }
            let FrameAction::Call {
                code_address,
                target,
                call_type,
                ..
            } = frame.action
            else {
                unreachable!()
            };
            if !supported.contains(&code_address) {
                return Err(
                    super::CoreSpaceProtocolError::unsupported_operation(format!(
                        "no protocol rule for internal contract {code_address:?}"
                    ))
                    .into(),
                );
            }
            if code_address != target
                || !matches!(
                    call_type,
                    cfx_vm_types::CallType::Call | cfx_vm_types::CallType::StaticCall
                )
            {
                return Err(super::CoreSpaceProtocolError::unsupported_operation(
                    "internal contract effects require a canonical native call",
                )
                .into());
            }
        }
        let changes = derive_protocol_changes(
            view.execution,
            view.state(),
            &self.currency,
            &self.espace_currency,
        )?;
        Ok(AnalysisReport::new(changes).explain(scope.clone(), SupportEvidence::Protocol))
    }
}

use std::sync::Arc;

use cfx_types::{AddressSpaceUtil, Space};
use simulation_core::{analysis::*, changes::ChangePosition};

use super::{
    CoreSpaceAnalysisError, CoreSpaceBlockContext, CoreSpaceChangeSet,
    CoreSpaceExecutedTransaction, CoreSpaceNativeCurrency, CoreSpaceStateAccess,
    CoreSpaceTypedTransaction, changes::derive_protocol_changes,
};
use crate::{
    execution::{FrameAction, TraceEvent},
    primitive::{address_from_cfx, address_to_cfx},
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

pub(crate) fn default_registry(
    currency: CoreSpaceNativeCurrency,
    espace_currency: crate::espace::EspaceNativeCurrency,
) -> CoreSpaceAnalyzerRegistry {
    let mut rules: Vec<Arc<dyn Analyzer<CoreSpaceAnalysisDomain>>> =
        vec![Arc::new(ContractAnalyzer)];
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
        Ok(candidates.select(|fact| {
            matches!(fact.kind, FactKind::NativeMovement | FactKind::Protocol)
                || (fact.chain.space == ExecutionSpace::Core && fact.kind == FactKind::Destroy)
                || (fact.chain.space == ExecutionSpace::Core
                    && matches!(
                        fact.kind,
                        FactKind::Call | FactKind::Log | FactKind::StorageWrite
                    )
                    && fact.address.is_some_and(|address| {
                        let address = address_to_cfx(address);
                        supported.contains(&address)
                            && view.execution.is_active_internal_contract(address)
                    }))
        }))
    }
    fn analyze<'a>(
        &self,
        view: CoreSpaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, CoreSpaceChangeSet>, CoreSpaceAnalysisError> {
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

struct ContractAnalyzer;
impl Analyzer<CoreSpaceAnalysisDomain> for ContractAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "conflux-contracts",
            layer: AnalyzerLayer::General,
            priority: 0,
            deployments: Vec::new(),
        }
    }
    fn select<'a>(
        &self,
        _: CoreSpaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisScope<'a>, CoreSpaceAnalysisError> {
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
        view: CoreSpaceAnalysisView<'_>,
        scope: &AnalysisScope<'a>,
    ) -> Result<AnalysisReport<'a, CoreSpaceChangeSet>, CoreSpaceAnalysisError> {
        use super::CoreSpaceProtocolError;
        if let Some(fact) = scope.facts().find(|fact| fact.kind != FactKind::Call) {
            return Err(CoreSpaceProtocolError::unsupported_operation(format!(
                "no reviewed contract implementation for {:?} at {:?}",
                fact.kind, fact.position,
            ))
            .into());
        }
        let frames: std::collections::BTreeSet<_> =
            scope.facts().filter_map(|fact| fact.frame_id).collect();
        for (frame_id, frame) in view.execution.trace().frames() {
            if !frames.contains(&frame_id.index()) {
                continue;
            }
            let FrameAction::Call { code_address, .. } = frame.action else {
                continue;
            };
            if frame.space == Space::Native
                && view.execution.is_active_internal_contract(code_address)
            {
                return Err(CoreSpaceProtocolError::unsupported_operation(format!(
                    "unverified internal contract call at {code_address:?}"
                ))
                .into());
            }
            for reader in [view.state().initial(), view.state().finalized()] {
                let code = reader
                    .code(code_address.with_space(frame.space))
                    .map_err(|source| {
                        CoreSpaceProtocolError::state_access(
                            "check contract implementation",
                            source,
                        )
                    })?;
                if code.is_some_and(|code| !code.is_empty()) {
                    return Err(CoreSpaceProtocolError::unsupported_operation(format!(
                        "no reviewed implementation for code at {code_address:?} in {:?}",
                        frame.space
                    ))
                    .into());
                }
            }
        }
        Ok(AnalysisReport::new(CoreSpaceChangeSet::new())
            .explain(scope.clone(), SupportEvidence::NoRelevantEffects))
    }
}

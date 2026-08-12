use std::collections::BTreeSet;

use alloy_primitives::Address;
use cfx_executor::machine::Machine;
use cfx_types::{AddressWithSpace, Space};
use cfx_vm_types::Spec;
use primitives::receipt::StorageChange;

use super::{
    CfxOperation, CfxOperations, SponsorshipOperation,
    basic::CoreSpaceOperationCollector,
    cross_space::collect_cross_space_call,
    sponsorship::{
        CollectedSponsorshipCall, collect_admin_change_attempt, collect_sponsorship_call,
        collect_standalone_sponsorship_refund, collect_storage_point_conversion,
    },
};
use crate::{
    core_space::CoreSpaceChangesError,
    execution::{CommittedExecutionTrace, FrameAction, TraceEvent},
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug)]
struct CfxOperationCollector {
    operations: Vec<CfxOperation>,
    core_space: CoreSpaceOperationCollector,
}

pub(crate) fn collect_cfx_operations(
    trace: &CommittedExecutionTrace,
    contracts_created: &[AddressWithSpace],
    storage_released: &[StorageChange],
    machine: &Machine,
    spec: &Spec,
) -> Result<CfxOperations, CoreSpaceChangesError> {
    let mut collector = CfxOperationCollector::new(storage_released)?;
    let mut contracts_with_admin_change_attempts = BTreeSet::new();
    for event in trace.events() {
        let TraceEvent::FrameStart { frame_id, .. } = event else {
            continue;
        };
        let Some(attempt) = collect_admin_change_attempt(trace, *frame_id, machine, spec)? else {
            continue;
        };
        if attempt.is_destroy && !spec.cip131 {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "pre-CIP-131 Core Space contract destruction may delete sponsorship access-rule entries that public RPC cannot enumerate",
            ));
        }
        contracts_with_admin_change_attempts.insert(attempt.contract_address);
    }

    let mut claimed_internal_transfers = BTreeSet::new();
    for event in trace.events() {
        if claimed_internal_transfers.contains(&event.position()) {
            continue;
        }
        match event {
            TraceEvent::FrameStart { position, frame_id } => {
                let frame = trace.frame(*frame_id);
                if let Some((operation, claimed_transfer_positions)) =
                    collect_cross_space_call(trace, *position, *frame_id, machine, spec)?
                {
                    collector
                        .operations
                        .push(CfxOperation::CrossSpace(operation));
                    claimed_internal_transfers.extend(claimed_transfer_positions);
                    continue;
                }
                if let Some((collected_call, claimed_transfer_positions)) =
                    collect_sponsorship_call(trace, *position, *frame_id, machine, spec)?
                {
                    match collected_call {
                        CollectedSponsorshipCall::Funding(operation) => collector.operations.push(
                            CfxOperation::Sponsorship(SponsorshipOperation::Funding(*operation)),
                        ),
                        CollectedSponsorshipCall::AccessRuleUpdates(updates) => collector
                            .operations
                            .extend(updates.into_iter().map(|update| {
                                CfxOperation::Sponsorship(SponsorshipOperation::AccessRule(update))
                            })),
                    }
                    claimed_internal_transfers.extend(claimed_transfer_positions);
                    continue;
                }
                match &frame.action {
                    FrameAction::Call { .. } => {
                        if let Some(operation) = collector.core_space.collect_call_value_transfer(
                            trace, *position, *frame_id, machine, spec,
                        )? {
                            collector.operations.push(CfxOperation::Basic(operation));
                        }
                    }
                    FrameAction::Create {
                        creator,
                        created_address,
                        value,
                    } => {
                        if let Some(operation) = collector.core_space.collect_create_value_transfer(
                            *position,
                            frame.space,
                            *creator,
                            *created_address,
                            u256_from_cfx(*value),
                        ) {
                            collector.operations.push(CfxOperation::Basic(operation));
                        }
                    }
                }
            }
            TraceEvent::InternalTransfer { .. } => {
                if let Some((conversion, claimed_transfer_positions)) =
                    collect_storage_point_conversion(trace, event)?
                {
                    collector.operations.push(CfxOperation::Sponsorship(
                        SponsorshipOperation::StoragePointConversion(conversion),
                    ));
                    claimed_internal_transfers.extend(claimed_transfer_positions);
                    continue;
                }
                if let Some(refund) = collect_standalone_sponsorship_refund(event)? {
                    collector.operations.push(CfxOperation::Sponsorship(
                        SponsorshipOperation::StandaloneRefund(refund),
                    ));
                    continue;
                }
                let (collected, claimed_transfer_positions) = collector
                    .core_space
                    .collect_internal_transfer(trace, event)?;
                if let Some(operation) = collected {
                    collector.operations.push(CfxOperation::Basic(operation));
                }
                claimed_internal_transfers.extend(claimed_transfer_positions);
            }
            TraceEvent::Log { .. } => {}
        }
    }

    collector.validate_sponsorship_access_admin_context(
        &contracts_with_admin_change_attempts,
        contracts_created,
    )?;
    collector.into_operations()
}

impl CfxOperationCollector {
    fn new(storage_released: &[StorageChange]) -> Result<Self, CoreSpaceChangesError> {
        Ok(Self {
            operations: Vec::new(),
            core_space: CoreSpaceOperationCollector::new(storage_released)?,
        })
    }

    fn into_operations(mut self) -> Result<CfxOperations, CoreSpaceChangesError> {
        for operation in self.core_space.finish()? {
            self.operations.push(CfxOperation::Basic(operation));
        }
        Ok(CfxOperations::from_operations(self.operations))
    }

    fn validate_sponsorship_access_admin_context(
        &self,
        contracts_with_admin_change_attempts: &BTreeSet<Address>,
        contracts_created: &[AddressWithSpace],
    ) -> Result<(), CoreSpaceChangesError> {
        let created_native_contracts: BTreeSet<_> = contracts_created
            .iter()
            .filter(|created| created.space == Space::Native)
            .map(|created| address_from_cfx(created.address))
            .collect();

        for operation in &self.operations {
            let CfxOperation::Sponsorship(SponsorshipOperation::AccessRule(update)) = operation
            else {
                continue;
            };
            if update.caller_role != super::SponsorshipAccessCallerRole::ContractAdmin {
                continue;
            }
            if contracts_with_admin_change_attempts.contains(&update.contract_address)
                || created_native_contracts.contains(&update.contract_address)
            {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space sponsorship access-rule admin was not stable during the transaction for contract {}",
                    update.contract_address
                )));
            }
        }
        Ok(())
    }
}

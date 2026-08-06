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
    ConfluxSimulationError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug)]
struct CfxOperationCollector {
    operations: Vec<CfxOperation>,
    core_space: CoreSpaceOperationCollector,
}

pub(crate) fn collect_cfx_operations(
    observations: &[Observation],
    contracts_created: &[AddressWithSpace],
    storage_released: &[StorageChange],
    machine: &Machine,
    spec: &Spec,
) -> Result<CfxOperations, ConfluxSimulationError> {
    let mut collector = CfxOperationCollector::new(storage_released)?;
    let mut contracts_with_admin_change_attempts = BTreeSet::new();
    for observation in observations {
        let Some(attempt) = collect_admin_change_attempt(observation, machine, spec)? else {
            continue;
        };
        if attempt.is_destroy && !spec.cip131 {
            return Err(ConfluxSimulationError::analysis_failed(
                "pre-CIP-131 Core Space contract destruction may delete sponsorship access-rule entries that public RPC cannot enumerate",
            ));
        }
        contracts_with_admin_change_attempts.insert(attempt.contract_address);
    }

    let mut observation_index = 0;
    while observation_index < observations.len() {
        let observation = &observations[observation_index];
        match observation {
            Observation::Call { .. } => {
                if let Some((operation, consumed)) =
                    collect_cross_space_call(observations, observation_index, machine, spec)?
                {
                    collector
                        .operations
                        .push(CfxOperation::CrossSpace(operation));
                    observation_index = advance_observation_index(observation_index, consumed)?;
                    continue;
                }
                if let Some((collected_call, consumed)) =
                    collect_sponsorship_call(observations, observation_index, machine, spec)?
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
                    observation_index = advance_observation_index(observation_index, consumed)?;
                    continue;
                }
                if let Some(operation) =
                    collector
                        .core_space
                        .collect_call_value_transfer(observation, machine, spec)?
                {
                    collector.operations.push(CfxOperation::Basic(operation));
                }
                observation_index = advance_observation_index(observation_index, 1)?;
            }
            Observation::CreateTransfer {
                position,
                space,
                from,
                to,
                value,
            } => {
                if let Some(operation) = collector.core_space.collect_create_value_transfer(
                    *position,
                    *space,
                    *from,
                    *to,
                    u256_from_cfx(*value),
                ) {
                    collector.operations.push(CfxOperation::Basic(operation));
                }
                observation_index = advance_observation_index(observation_index, 1)?;
            }
            Observation::InternalTransfer { .. } => {
                if let Some((conversion, consumed)) =
                    collect_storage_point_conversion(observations, observation_index)?
                {
                    collector.operations.push(CfxOperation::Sponsorship(
                        SponsorshipOperation::StoragePointConversion(conversion),
                    ));
                    observation_index = advance_observation_index(observation_index, consumed)?;
                    continue;
                }
                if let Some(refund) = collect_standalone_sponsorship_refund(
                    observations.get(observation_index).ok_or_else(|| {
                        ConfluxSimulationError::analysis_failed(
                            "Core Space CFX collector received an inconsistent observation index",
                        )
                    })?,
                )? {
                    collector.operations.push(CfxOperation::Sponsorship(
                        SponsorshipOperation::StandaloneRefund(refund),
                    ));
                    observation_index = advance_observation_index(observation_index, 1)?;
                    continue;
                }
                let (operation, consumed) = collector
                    .core_space
                    .collect_internal_transfer(observations, observation_index)?;
                if let Some(operation) = operation {
                    collector.operations.push(CfxOperation::Basic(operation));
                }
                observation_index = advance_observation_index(observation_index, consumed)?;
            }
            Observation::Log { .. } => {
                observation_index = advance_observation_index(observation_index, 1)?;
            }
        }
    }

    collector.validate_sponsorship_access_admin_context(
        &contracts_with_admin_change_attempts,
        contracts_created,
    )?;
    collector.into_operations()
}

fn advance_observation_index(
    observation_index: usize,
    consumed: usize,
) -> Result<usize, ConfluxSimulationError> {
    observation_index.checked_add(consumed).ok_or_else(|| {
        ConfluxSimulationError::analysis_failed(
            "Core Space CFX observation index overflowed during collection",
        )
    })
}

impl CfxOperationCollector {
    fn new(storage_released: &[StorageChange]) -> Result<Self, ConfluxSimulationError> {
        Ok(Self {
            operations: Vec::new(),
            core_space: CoreSpaceOperationCollector::new(storage_released)?,
        })
    }

    fn into_operations(mut self) -> Result<CfxOperations, ConfluxSimulationError> {
        for operation in self.core_space.finish()? {
            self.operations.push(CfxOperation::Basic(operation));
        }
        Ok(CfxOperations::from_operations(self.operations))
    }

    fn validate_sponsorship_access_admin_context(
        &self,
        contracts_with_admin_change_attempts: &BTreeSet<Address>,
        contracts_created: &[AddressWithSpace],
    ) -> Result<(), ConfluxSimulationError> {
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
                return Err(ConfluxSimulationError::analysis_failed(format!(
                    "Core Space sponsorship access-rule admin was not stable during the transaction for contract {}",
                    update.contract_address
                )));
            }
        }
        Ok(())
    }
}

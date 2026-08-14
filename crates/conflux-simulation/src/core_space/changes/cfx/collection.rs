use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::Address;
use cfx_executor::machine::Machine;
use cfx_types::Space;
use cfx_vm_types::Spec;
use primitives::receipt::StorageChange;

use super::{
    AdminOperation, BasicCfxOperation, CfxOperation, CfxOperations, SponsorshipOperation,
    basic::CoreSpaceOperationCollector,
    cross_space::collect_cross_space_call,
    sponsorship::{
        CollectedAdminCall, CollectedSponsorshipCall, collect_admin_call, collect_sponsorship_call,
        collect_standalone_sponsorship_refund, collect_storage_point_conversion,
    },
};
use crate::{
    core_space::CoreSpaceChangesError,
    core_space::changes::staking::CommittedStakingCall,
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
    storage_released: &[StorageChange],
    machine: &Machine,
    spec: &Spec,
    transaction_sender: Address,
    committed_staking_calls: &[CommittedStakingCall],
) -> Result<CfxOperations, CoreSpaceChangesError> {
    let mut collector = CfxOperationCollector::new(storage_released)?;
    let mut owned_transfer_positions = BTreeSet::new();
    let mut staking_ops = BTreeMap::new();
    for call in committed_staking_calls {
        for transfer_position in call.owned_transfer_positions() {
            if !owned_transfer_positions.insert(transfer_position) {
                return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                    "Core Space transfer at trace position {transfer_position} was owned by multiple staking operations"
                )));
            }
        }
        let staking_balance_operation = match *call {
            CommittedStakingCall::Deposit {
                account, amount, ..
            } => Some(BasicCfxOperation::StakingDeposit { account, amount }),
            CommittedStakingCall::Withdrawal {
                account,
                principal_amount,
                reward_amount,
                ..
            } => Some(BasicCfxOperation::StakingWithdrawal {
                account,
                principal_amount,
                reward_amount,
            }),
            CommittedStakingCall::VoteLock { .. } => None,
        };
        if let Some(staking_balance_operation) = staking_balance_operation
            && staking_ops
                .insert(call.frame_position(), staking_balance_operation)
                .is_some()
        {
            return Err(CoreSpaceChangesError::inconsistent_execution(
                "multiple Core Space staking operations used the same trace position",
            ));
        }
    }
    for event in trace.events() {
        if owned_transfer_positions.contains(&event.position()) {
            continue;
        }
        match event {
            TraceEvent::FrameStart { position, frame_id } => {
                if let Some(operation) = staking_ops.remove(position) {
                    collector.operations.push(CfxOperation::Basic(operation));
                    continue;
                }
                let frame = trace.frame(*frame_id);
                if let Some(admin_call) =
                    collect_admin_call(trace, *position, *frame_id, machine, spec)?
                {
                    match admin_call {
                        CollectedAdminCall::Set(operation) => collector
                            .operations
                            .push(CfxOperation::Admin(AdminOperation::Set(operation))),
                        CollectedAdminCall::Destroy { contract_address } if !spec.cip131 => {
                            return Err(CoreSpaceChangesError::inconsistent_execution(format!(
                                "pre-CIP-131 destruction of Core Space contract {contract_address} may delete sponsorship access-rule entries that public RPC cannot enumerate"
                            )));
                        }
                        CollectedAdminCall::Destroy {
                            contract_address: _,
                        } => {}
                    }
                    continue;
                }
                if let Some((operation, claimed_transfer_positions)) =
                    collect_cross_space_call(trace, *position, *frame_id, machine, spec)?
                {
                    collector
                        .operations
                        .push(CfxOperation::CrossSpace(operation));
                    owned_transfer_positions.extend(claimed_transfer_positions);
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
                    owned_transfer_positions.extend(claimed_transfer_positions);
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
                        if frame.space == Space::Native {
                            collector.operations.push(CfxOperation::Admin(
                                AdminOperation::Initialize {
                                    contract_address: address_from_cfx(*created_address),
                                    admin: transaction_sender,
                                },
                            ));
                        }
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
                    owned_transfer_positions.extend(claimed_transfer_positions);
                    continue;
                }
                if let Some(refund) = collect_standalone_sponsorship_refund(event)? {
                    collector.operations.push(CfxOperation::Sponsorship(
                        SponsorshipOperation::StandaloneRefund(refund),
                    ));
                    continue;
                }
                let (collected, claimed_transfer_positions) =
                    collector.core_space.collect_internal_transfer(event)?;
                if let Some(operation) = collected {
                    collector.operations.push(CfxOperation::Basic(operation));
                }
                owned_transfer_positions.extend(claimed_transfer_positions);
            }
            TraceEvent::Log { .. } => {}
        }
    }

    if !staking_ops.is_empty() {
        return Err(CoreSpaceChangesError::inconsistent_execution(
            "a committed Core Space staking operation had no matching frame position",
        ));
    }

    collector.into_operations(committed_staking_calls.iter().map(|call| call.account()))
}

impl CfxOperationCollector {
    fn new(storage_released: &[StorageChange]) -> Result<Self, CoreSpaceChangesError> {
        Ok(Self {
            operations: Vec::new(),
            core_space: CoreSpaceOperationCollector::new(storage_released)?,
        })
    }

    fn into_operations(
        mut self,
        staking_accounts: impl IntoIterator<Item = Address>,
    ) -> Result<CfxOperations, CoreSpaceChangesError> {
        for operation in self.core_space.finish()? {
            self.operations.push(CfxOperation::Basic(operation));
        }
        Ok(CfxOperations::from_operations(
            self.operations,
            staking_accounts,
        ))
    }
}

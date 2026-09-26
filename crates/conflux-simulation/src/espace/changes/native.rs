use alloy_primitives::{Address, U256};

use crate::espace::EspaceAnalysisError;
use crate::espace::{
    EspaceCallKind, EspaceExecutedTransaction, EspaceExecutionSpace, EspaceFrameAction,
    EspaceTransferPocket,
};
use simulation_core::{
    analysis::{AnalysisScope, FactKind},
    changes::ChangePosition,
};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
struct NativeOperationCollector {
    operations: Vec<NativeOperation>,
}

pub(crate) fn collect_native_operations(
    execution: &EspaceExecutedTransaction,
    scope: &AnalysisScope<'_>,
) -> Result<Vec<NativeOperation>, EspaceAnalysisError> {
    let positions: BTreeSet<_> = scope
        .facts()
        .filter(|fact| fact.kind == FactKind::NativeMovement)
        .map(|fact| fact.position)
        .collect();
    let mut collector = NativeOperationCollector::default();

    for frame in execution.committed_frames() {
        if frame.space() != EspaceExecutionSpace::Espace
            || !positions.contains(&ChangePosition::Execution(frame.position().index()))
        {
            continue;
        }
        match frame.action() {
            EspaceFrameAction::Call {
                kind: EspaceCallKind::Call,
                caller,
                target,
                value,
                ..
            } => {
                collector.push_account_transfer(frame.position().index(), *caller, *target, *value)
            }
            EspaceFrameAction::Create {
                creator,
                actual_address,
                value,
                ..
            } => collector.push_account_transfer(
                frame.position().index(),
                *creator,
                *actual_address,
                *value,
            ),
            EspaceFrameAction::Call { .. } => {}
        }
    }
    for transfer in execution.internal_transfers() {
        if !positions.contains(&ChangePosition::Execution(transfer.position().index())) {
            continue;
        }
        collector.collect_internal_transfer(
            transfer.position().index(),
            transfer.from(),
            transfer.to(),
            transfer.value(),
        )?;
    }

    collector.operations.sort_by_key(NativeOperation::position);
    Ok(collector.operations)
}

impl NativeOperationCollector {
    fn collect_internal_transfer(
        &mut self,
        position: usize,
        from: EspaceTransferPocket,
        to: EspaceTransferPocket,
        amount: U256,
    ) -> Result<(), EspaceAnalysisError> {
        if amount.is_zero() {
            return Ok(());
        }

        let source = espace_balance_account(from);
        let destination = espace_balance_account(to);
        match (source, destination, from, to) {
            (Some(from), Some(to), _, _) => self.push_account_transfer(position, from, to, amount),
            (Some(_), None, _, EspaceTransferPocket::GasPayment)
            | (None, Some(_), EspaceTransferPocket::GasPayment, _) => {}
            (Some(contract), None, _, EspaceTransferPocket::MintBurn) => {
                self.operations.push(NativeOperation::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                });
            }
            (None, None, from, to) if !involves_non_espace_balance(from, to) => {}
            _ => {
                return Err(EspaceAnalysisError::Unsupported {
                    details: format!(
                        "native effect {from:?} -> {to:?} is outside the eSpace native-change scope"
                    ),
                });
            }
        }

        Ok(())
    }

    fn push_account_transfer(&mut self, position: usize, from: Address, to: Address, amount: U256) {
        if !amount.is_zero() && from != to {
            self.operations.push(NativeOperation::AccountTransfer {
                position,
                from,
                to,
                amount,
            });
        }
    }
}

fn espace_balance_account(pocket: EspaceTransferPocket) -> Option<Address> {
    match pocket {
        EspaceTransferPocket::EspaceBalance(address) => Some(address),
        _ => None,
    }
}

fn involves_non_espace_balance(from: EspaceTransferPocket, to: EspaceTransferPocket) -> bool {
    [from, to].into_iter().any(|pocket| {
        matches!(
            pocket,
            EspaceTransferPocket::CoreBalance(_)
                | EspaceTransferPocket::StakingBalance(_)
                | EspaceTransferPocket::StorageCollateral(_)
                | EspaceTransferPocket::SponsorBalanceForGas(_)
                | EspaceTransferPocket::SponsorBalanceForStorage(_)
        )
    })
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeOperation {
    AccountTransfer {
        position: usize,
        from: Address,
        to: Address,
        amount: U256,
    },
    SelfDestructBurn {
        position: usize,
        contract: Address,
        amount: U256,
    },
}
impl NativeOperation {
    pub(crate) fn position(&self) -> usize {
        match self {
            Self::AccountTransfer { position, .. } | Self::SelfDestructBurn { position, .. } => {
                *position
            }
        }
    }
}

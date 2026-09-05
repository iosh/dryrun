use alloy_primitives::{Address, U256};

use super::{NativeOperation, NativeOperations, NativeResolverDiagnostic};
use crate::espace::{
    EspaceExecutedTransaction, EspaceExecutionSpace, EspaceFrameAction, EspaceTransferPocket,
};

#[derive(Debug, Default)]
struct NativeOperationCollector {
    operations: Vec<NativeOperation>,
}

pub(super) fn collect_native_operations(
    execution: &EspaceExecutedTransaction,
) -> Result<NativeOperations, NativeResolverDiagnostic> {
    let mut collector = NativeOperationCollector::default();

    for frame in execution.committed_frames() {
        if frame.space() != EspaceExecutionSpace::Espace {
            continue;
        }
        match frame.action() {
            EspaceFrameAction::Call {
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
        }
    }
    for transfer in execution.internal_transfers() {
        collector.collect_internal_transfer(
            transfer.position().index(),
            transfer.from(),
            transfer.to(),
            transfer.value(),
        )?;
    }

    Ok(NativeOperations::from_operations(collector.operations))
}

impl NativeOperationCollector {
    fn collect_internal_transfer(
        &mut self,
        position: usize,
        from: EspaceTransferPocket,
        to: EspaceTransferPocket,
        amount: U256,
    ) -> Result<(), NativeResolverDiagnostic> {
        if amount.is_zero() {
            return Ok(());
        }

        let source = espace_balance_account(from);
        let destination = espace_balance_account(to);
        match (source, destination, from, to) {
            (Some(from), Some(to), _, _) => self.push_account_transfer(position, from, to, amount),
            (Some(payer), None, _, EspaceTransferPocket::GasPayment) => {
                self.operations.push(NativeOperation::GasPrecharge {
                    position,
                    payer,
                    amount,
                });
            }
            (None, Some(recipient), EspaceTransferPocket::GasPayment, _) => {
                self.operations.push(NativeOperation::GasRefund {
                    position,
                    recipient,
                    amount,
                });
            }
            (Some(contract), None, _, EspaceTransferPocket::MintBurn) => {
                self.operations.push(NativeOperation::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                });
            }
            (None, None, from, to) if !involves_non_espace_balance(from, to) => {}
            _ => {
                return Err(NativeResolverDiagnostic::new(format!(
                    "native effect {from:?} -> {to:?} is outside the eSpace resolver scope"
                )));
            }
        }

        Ok(())
    }

    fn push_account_transfer(&mut self, position: usize, from: Address, to: Address, amount: U256) {
        if !amount.is_zero() {
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

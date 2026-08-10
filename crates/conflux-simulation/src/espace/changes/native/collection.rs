use alloy_primitives::{Address, U256};
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::Space;

use super::{NativeOperation, NativeOperations};
use crate::{
    espace::EspaceNativeChangeError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug, Default)]
struct NativeOperationCollector {
    operations: Vec<NativeOperation>,
}

pub(super) fn collect_native_operations(
    observations: &[Observation],
) -> Result<NativeOperations, EspaceNativeChangeError> {
    let mut collector = NativeOperationCollector::default();

    for observation in observations {
        match observation {
            Observation::Call {
                position,
                space: Space::Ethereum,
                caller,
                target,
                transferred_value,
                ..
            } => collector.push_account_transfer(
                *position,
                address_from_cfx(*caller),
                address_from_cfx(*target),
                u256_from_cfx(*transferred_value),
            ),
            Observation::CreateTransfer {
                position,
                space: Space::Ethereum,
                from,
                to,
                value,
            } => collector.push_account_transfer(
                *position,
                address_from_cfx(*from),
                address_from_cfx(*to),
                u256_from_cfx(*value),
            ),
            Observation::InternalTransfer {
                position,
                from,
                to,
                value,
                ..
            } => collector.collect_internal_transfer(*position, from, to, u256_from_cfx(*value))?,
            Observation::Call { .. }
            | Observation::CreateTransfer { .. }
            | Observation::Log { .. } => {}
        }
    }

    Ok(NativeOperations::from_operations(collector.operations))
}

impl NativeOperationCollector {
    fn collect_internal_transfer(
        &mut self,
        position: usize,
        from: &AddressPocket,
        to: &AddressPocket,
        amount: U256,
    ) -> Result<(), EspaceNativeChangeError> {
        if amount.is_zero() {
            return Ok(());
        }

        let source = espace_balance_account(from);
        let destination = espace_balance_account(to);
        match (source, destination, from, to) {
            (Some(from), Some(to), _, _) => self.push_account_transfer(position, from, to, amount),
            (Some(payer), None, _, AddressPocket::GasPayment) => {
                self.operations
                    .push(NativeOperation::GasPrecharge { payer, amount });
            }
            (None, Some(recipient), AddressPocket::GasPayment, _) => {
                self.operations
                    .push(NativeOperation::GasRefund { recipient, amount });
            }
            (Some(contract), None, _, AddressPocket::MintBurn) => {
                self.operations.push(NativeOperation::SelfDestructBurn {
                    position,
                    contract,
                    amount,
                });
            }
            (None, Some(_), AddressPocket::MintBurn, _) => {
                return Err(EspaceNativeChangeError::UnsupportedBalanceOperation {
                    details: format!("{} -> {}", from.pocket(), to.pocket()),
                });
            }
            _ if involves_non_espace_balance(from, to) => {
                return Err(EspaceNativeChangeError::UnsupportedCrossSpaceMovement {
                    details: format!(
                        "{}:{} -> {}:{}",
                        from.space(),
                        from.pocket(),
                        to.space(),
                        to.pocket()
                    ),
                });
            }
            (Some(_), None, _, _) | (None, Some(_), _, _) => {
                return Err(EspaceNativeChangeError::UnsupportedBalanceOperation {
                    details: format!("{} -> {}", from.pocket(), to.pocket()),
                });
            }
            (None, None, _, _) => {}
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

fn espace_balance_account(pocket: &AddressPocket) -> Option<Address> {
    match pocket {
        AddressPocket::Balance(address) if address.space == Space::Ethereum => {
            Some(address_from_cfx(address.address))
        }
        _ => None,
    }
}

fn involves_non_espace_balance(from: &AddressPocket, to: &AddressPocket) -> bool {
    [from, to].into_iter().any(|pocket| {
        matches!(pocket, AddressPocket::Balance(address) if address.space != Space::Ethereum)
    })
}

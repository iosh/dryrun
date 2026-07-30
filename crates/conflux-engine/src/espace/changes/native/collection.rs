use alloy_primitives::{Address, U256};
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::Space;
use contract_standards::Position;

use super::{NativeEvidence, NativeOperation};
use crate::{
    ConfluxEngineError,
    execution::Observation,
    primitive::{address_from_cfx, u256_from_cfx},
};

#[derive(Debug, Default)]
struct NativeCollector {
    operations: Vec<NativeOperation>,
}

pub(crate) fn collect_native_evidence(
    observations: &[Observation],
) -> Result<NativeEvidence, ConfluxEngineError> {
    let mut collector = NativeCollector::default();

    for observation in observations {
        match observation {
            Observation::Call {
                position,
                space: Space::Ethereum,
                caller,
                target,
                transferred_value,
                ..
            } => collector.push_transfer(
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
            } => collector.push_transfer(
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

    Ok(collector.finish())
}

impl NativeCollector {
    fn finish(self) -> NativeEvidence {
        NativeEvidence::from_operations(self.operations)
    }

    fn collect_internal_transfer(
        &mut self,
        position: usize,
        from: &AddressPocket,
        to: &AddressPocket,
        amount: U256,
    ) -> Result<(), ConfluxEngineError> {
        let from_balance = espace_balance(from);
        let to_balance = espace_balance(to);

        match (from_balance, to_balance, from, to) {
            (Some(from), Some(to), _, _) => self.push_transfer(position, from, to, amount),
            (Some(payer), None, _, AddressPocket::GasPayment) => {
                self.push_gas_precharge(payer, amount);
            }
            (None, Some(recipient), AddressPocket::GasPayment, _) => {
                self.push_gas_refund(recipient, amount);
            }
            (Some(_), None, _, AddressPocket::MintBurn)
            | (None, Some(_), AddressPocket::MintBurn, _)
                if amount.is_zero() => {}
            (Some(_), None, _, AddressPocket::Balance(_))
            | (None, Some(_), AddressPocket::Balance(_), _) => {
                return Err(ConfluxEngineError::analysis_failed(
                    "cross-space native balance movement is not supported by eSpace changes",
                ));
            }
            (Some(_), None, _, _) | (None, Some(_), _, _) => {
                return Err(ConfluxEngineError::analysis_failed(format!(
                    "eSpace native balance movement used unsupported {} -> {} pockets",
                    from.pocket(),
                    to.pocket()
                )));
            }
            (None, None, _, _) => {}
        }

        Ok(())
    }

    fn push_transfer(&mut self, position: usize, from: Address, to: Address, amount: U256) {
        if amount.is_zero() {
            return;
        }

        self.operations.push(NativeOperation::Transfer {
            position: Position::new(position, 0),
            from,
            to,
            amount,
        });
    }

    fn push_gas_precharge(&mut self, payer: Address, amount: U256) {
        if amount.is_zero() {
            return;
        }

        self.operations
            .push(NativeOperation::GasPrecharge { payer, amount });
    }

    fn push_gas_refund(&mut self, recipient: Address, amount: U256) {
        if amount.is_zero() {
            return;
        }

        self.operations
            .push(NativeOperation::GasRefund { recipient, amount });
    }
}

fn espace_balance(pocket: &AddressPocket) -> Option<Address> {
    match pocket {
        AddressPocket::Balance(address) if address.space == Space::Ethereum => {
            Some(address_from_cfx(address.address))
        }
        _ => None,
    }
}

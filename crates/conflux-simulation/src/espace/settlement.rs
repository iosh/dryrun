use alloy_primitives::U256;
use cfx_executor::executive_observer::AddressPocket;
use cfx_types::Space;

use super::EspaceResultIntegrationError;
use crate::{
    execution::{ConfluxExecutionOutput, TraceEvent},
    primitive::u256_from_cfx,
};

pub(crate) fn verify_fee_settlement(
    output: &ConfluxExecutionOutput,
) -> Result<(), EspaceResultIntegrationError> {
    let mut precharge = U256::ZERO;
    let mut refund = U256::ZERO;

    for event in output.trace.events() {
        let TraceEvent::InternalTransfer {
            from, to, value, ..
        } = event
        else {
            continue;
        };
        let amount = u256_from_cfx(*value);

        match (from, to) {
            (fee_payer, AddressPocket::GasPayment) if is_espace_fee_payer(fee_payer) => {
                precharge = precharge.checked_add(amount).ok_or_else(|| {
                    EspaceResultIntegrationError::invalid_observed_fee_settlement(
                        "gas precharge accumulation overflowed U256",
                    )
                })?;
            }
            (AddressPocket::GasPayment, fee_payer) if is_espace_fee_payer(fee_payer) => {
                refund = refund.checked_add(amount).ok_or_else(|| {
                    EspaceResultIntegrationError::invalid_observed_fee_settlement(
                        "gas refund accumulation overflowed U256",
                    )
                })?;
            }
            _ => {}
        }
    }

    let settled_fee = precharge.checked_sub(refund).ok_or_else(|| {
        EspaceResultIntegrationError::invalid_observed_fee_settlement(format!(
            "refund {refund} exceeds precharge {precharge}"
        ))
    })?;
    if settled_fee != output.common.fee {
        return Err(
            EspaceResultIntegrationError::invalid_observed_fee_settlement(format!(
                "committed internal transfers settle {settled_fee}, executor reports {}",
                output.common.fee
            )),
        );
    }

    Ok(())
}

fn is_espace_fee_payer(pocket: &AddressPocket) -> bool {
    matches!(
        pocket,
        AddressPocket::Balance(address) if address.space == Space::Ethereum
    ) || matches!(pocket, AddressPocket::SponsorBalanceForGas(_))
}

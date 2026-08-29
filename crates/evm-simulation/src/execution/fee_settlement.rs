use alloy::primitives::U256;

use crate::{EvmBlobGasFee, EvmExecutionGasFee, EvmFee, EvmGas, EvmResultIntegrationError};

/// Internal balance-accounting facts for protocol fee settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvmFeeSettlement {
    fee: EvmFee,
}

impl EvmFeeSettlement {
    pub(super) fn new(
        gas: &EvmGas,
        effective_gas_price: u128,
        base_fee_per_gas: u64,
        blob_gas_fee: Option<EvmBlobGasFee>,
    ) -> Result<Self, EvmResultIntegrationError> {
        let effective_gas_price_u256 = U256::from(effective_gas_price);
        let gas_used = U256::from(gas.gas_used());
        let charged_amount = gas_used * effective_gas_price_u256;
        let burnt_amount = gas_used * U256::from(base_fee_per_gas);
        let execution_gas_fee =
            EvmExecutionGasFee::new(effective_gas_price, charged_amount, burnt_amount)?;
        Ok(Self {
            fee: EvmFee::new(execution_gas_fee, blob_gas_fee),
        })
    }

    pub(crate) fn into_fee(self) -> EvmFee {
        self.fee
    }
}

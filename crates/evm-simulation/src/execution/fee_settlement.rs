use alloy::primitives::U256;

use crate::{EvmBlobGasFee, EvmExecutionGasFee, EvmFee, EvmGas, EvmResultIntegrationError};

/// Internal balance-accounting facts for protocol fee settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvmFeeSettlement {
    fee: EvmFee,
    caller_precharge: U256,
    caller_refund: U256,
}

impl EvmFeeSettlement {
    pub(super) fn new(
        gas: &EvmGas,
        transaction_gas_limit: u64,
        effective_gas_price: u128,
        base_fee_per_gas: u64,
        blob_gas_fee: Option<EvmBlobGasFee>,
    ) -> Result<Self, EvmResultIntegrationError> {
        let effective_gas_price_u256 = U256::from(effective_gas_price);
        let gas_used = U256::from(gas.gas_used());
        let execution_precharge = U256::from(transaction_gas_limit) * effective_gas_price_u256;
        let charged_amount = gas_used * effective_gas_price_u256;
        let burnt_amount = gas_used * U256::from(base_fee_per_gas);
        let caller_refund = execution_precharge - charged_amount;
        let execution_gas_fee =
            EvmExecutionGasFee::new(effective_gas_price, charged_amount, burnt_amount)?;
        let caller_precharge = execution_precharge
            + blob_gas_fee
                .as_ref()
                .map_or(U256::ZERO, EvmBlobGasFee::charged_amount);

        Ok(Self {
            fee: EvmFee::new(execution_gas_fee, blob_gas_fee),
            caller_precharge,
            caller_refund,
        })
    }

    pub(crate) fn into_fee(self) -> EvmFee {
        self.fee
    }

    pub(crate) const fn caller_precharge(&self) -> U256 {
        self.caller_precharge
    }

    pub(crate) const fn caller_refund(&self) -> U256 {
        self.caller_refund
    }

    pub(crate) fn beneficiary_reward(&self) -> U256 {
        self.fee.beneficiary_reward()
    }
}

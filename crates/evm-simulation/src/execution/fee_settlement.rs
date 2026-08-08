use alloy::primitives::U256;

use crate::{EvmExecutionGasFee, EvmGas, EvmResultIntegrationError};

/// Internal balance-accounting facts for execution gas settlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvmFeeSettlement {
    execution_gas_fee: EvmExecutionGasFee,
    gas_precharge: U256,
    caller_refund: U256,
}

impl EvmFeeSettlement {
    pub(super) fn new(
        gas: &EvmGas,
        transaction_gas_limit: u64,
        effective_gas_price: u128,
        base_fee_per_gas: u64,
    ) -> Result<Self, EvmResultIntegrationError> {
        let effective_gas_price_u256 = U256::from(effective_gas_price);
        let gas_used = U256::from(gas.gas_used());
        let gas_precharge = U256::from(transaction_gas_limit) * effective_gas_price_u256;
        let charged_amount = gas_used * effective_gas_price_u256;
        let burnt_amount = gas_used * U256::from(base_fee_per_gas);
        let caller_refund = gas_precharge - charged_amount;
        let execution_gas_fee =
            EvmExecutionGasFee::new(effective_gas_price, charged_amount, burnt_amount)?;

        Ok(Self {
            execution_gas_fee,
            gas_precharge,
            caller_refund,
        })
    }

    pub(crate) fn into_execution_gas_fee(self) -> EvmExecutionGasFee {
        self.execution_gas_fee
    }

    pub(crate) const fn gas_precharge(&self) -> U256 {
        self.gas_precharge
    }

    pub(crate) const fn caller_refund(&self) -> U256 {
        self.caller_refund
    }

    pub(crate) fn beneficiary_reward(&self) -> U256 {
        self.execution_gas_fee.beneficiary_reward()
    }
}

use alloy::primitives::U256;
use revm::context_interface::result::{ExecutionResult, HaltReason};

use super::EvmExecutionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmFeeSettlement {
    pub fee: U256,
    pub burnt_fee: U256,
    pub gas_precharge: U256,
    pub caller_refund: U256,
    pub beneficiary_reward: U256,
}

impl EvmFeeSettlement {
    pub(super) fn new(
        result: &ExecutionResult<HaltReason>,
        effective_gas_price: u128,
        base_fee_per_gas: u64,
    ) -> Result<Self, EvmExecutionError> {
        let gas = result.gas();
        let gas_limit = U256::from(gas.limit());
        let gas_used = U256::from(gas.used());
        let effective_gas_price = U256::from(effective_gas_price);
        let base_fee_per_gas = U256::from(base_fee_per_gas);

        let gas_precharge = gas_limit
            .checked_mul(effective_gas_price)
            .ok_or(EvmExecutionError::FeeSettlement)?;
        let fee = gas_used
            .checked_mul(effective_gas_price)
            .ok_or(EvmExecutionError::FeeSettlement)?;
        let burnt_fee = gas_used
            .checked_mul(base_fee_per_gas)
            .ok_or(EvmExecutionError::FeeSettlement)?;
        let caller_refund = gas_precharge
            .checked_sub(fee)
            .ok_or(EvmExecutionError::FeeSettlement)?;
        let beneficiary_reward = fee
            .checked_sub(burnt_fee)
            .ok_or(EvmExecutionError::FeeSettlement)?;

        Ok(Self {
            fee,
            burnt_fee,
            gas_precharge,
            caller_refund,
            beneficiary_reward,
        })
    }
}

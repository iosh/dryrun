use alloy_primitives::U256;
use revm::context_interface::result::ResultGas;

use crate::EvmEngineError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransactionFeeSettlement {
    pub(crate) fee: U256,
    pub(crate) burnt_fee: U256,
    pub(crate) gas_precharge: U256,
    pub(crate) caller_refund: U256,
    pub(crate) beneficiary_reward: U256,
}

impl TransactionFeeSettlement {
    pub(crate) fn new(
        gas: &ResultGas,
        effective_gas_price: u128,
        base_fee_per_gas: u64,
    ) -> Result<Self, EvmEngineError> {
        let gas_limit = U256::from(gas.limit());
        let gas_used = U256::from(gas.used());
        let effective_gas_price = U256::from(effective_gas_price);
        let base_fee_per_gas = U256::from(base_fee_per_gas);

        let gas_precharge = gas_limit
            .checked_mul(effective_gas_price)
            .ok_or_else(fee_arithmetic_error)?;
        let fee = gas_used
            .checked_mul(effective_gas_price)
            .ok_or_else(fee_arithmetic_error)?;
        let burnt_fee = gas_used
            .checked_mul(base_fee_per_gas)
            .ok_or_else(fee_arithmetic_error)?;
        let caller_refund = gas_precharge
            .checked_sub(fee)
            .ok_or_else(fee_arithmetic_error)?;
        let beneficiary_reward = fee
            .checked_sub(burnt_fee)
            .ok_or_else(fee_arithmetic_error)?;

        Ok(Self {
            fee,
            burnt_fee,
            gas_precharge,
            caller_refund,
            beneficiary_reward,
        })
    }
}

fn fee_arithmetic_error() -> EvmEngineError {
    EvmEngineError::engine_execution_error("transaction fee settlement arithmetic was inconsistent")
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use revm::context_interface::result::ResultGas;

    use super::TransactionFeeSettlement;

    #[test]
    fn settles_components_from_final_gas_used() {
        let cases = [
            (
                "zero-base-fee",
                ResultGas::new(100, 70, 20, 0, 0),
                10_u128,
                0_u64,
                (500_u64, 0_u64, 1_000_u64, 500_u64, 500_u64),
            ),
            (
                "base-fee-split",
                ResultGas::new(100, 70, 20, 0, 0),
                10_u128,
                3_u64,
                (500_u64, 150_u64, 1_000_u64, 500_u64, 350_u64),
            ),
            (
                "floor-gas-applied",
                ResultGas::new(100, 70, 30, 60, 0),
                10_u128,
                2_u64,
                (600_u64, 120_u64, 1_000_u64, 400_u64, 480_u64),
            ),
        ];

        for (name, gas, effective_price, base_fee, expected) in cases {
            let settlement =
                TransactionFeeSettlement::new(&gas, effective_price, base_fee).expect(name);

            assert_eq!(settlement.fee, U256::from(expected.0), "{name}: fee");
            assert_eq!(
                settlement.burnt_fee,
                U256::from(expected.1),
                "{name}: burnt fee"
            );
            assert_eq!(
                settlement.gas_precharge,
                U256::from(expected.2),
                "{name}: gas precharge"
            );
            assert_eq!(
                settlement.caller_refund,
                U256::from(expected.3),
                "{name}: caller refund"
            );
            assert_eq!(
                settlement.beneficiary_reward,
                U256::from(expected.4),
                "{name}: beneficiary reward"
            );
        }
    }

    #[test]
    fn rejects_base_fee_above_effective_gas_price() {
        let gas = ResultGas::new(100, 100, 0, 0, 0);

        let error = TransactionFeeSettlement::new(&gas, 2, 3).unwrap_err();

        assert_eq!(error.kind_code(), Some("engine_execution_error"));
    }
}

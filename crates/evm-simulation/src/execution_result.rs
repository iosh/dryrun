use alloy_primitives::U256;

use crate::EvmResultIntegrationError;

/// Gas accounting produced by an executed EVM transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmGas {
    intrinsic_gas: u64,
    spent_before_refund: u64,
    refund_credit: u64,
    floor_gas: Option<u64>,
}

impl EvmGas {
    pub(crate) fn new(
        transaction_gas_limit: u64,
        result_gas_limit: u64,
        intrinsic_gas: u64,
        spent_before_refund: u64,
        refund_credit: u64,
        floor_gas: u64,
    ) -> Result<Self, EvmResultIntegrationError> {
        if result_gas_limit != transaction_gas_limit {
            return Err(EvmResultIntegrationError::GasLimitMismatch {
                transaction_gas_limit,
                result_gas_limit,
            });
        }

        if spent_before_refund > result_gas_limit
            || refund_credit > spent_before_refund
            || intrinsic_gas > spent_before_refund
            || floor_gas > result_gas_limit
        {
            return Err(EvmResultIntegrationError::InvalidGasAccounting {
                gas_limit: result_gas_limit,
                intrinsic_gas,
                spent_before_refund,
                refund_credit,
                floor_gas,
            });
        }

        Ok(Self {
            intrinsic_gas,
            spent_before_refund,
            refund_credit,
            floor_gas: (floor_gas != 0).then_some(floor_gas),
        })
    }

    /// Transaction overhead charged before EVM bytecode execution.
    pub const fn intrinsic_gas(&self) -> u64 {
        self.intrinsic_gas
    }

    /// Gas consumed before applying any refund credit.
    pub const fn spent_before_refund(&self) -> u64 {
        self.spent_before_refund
    }

    /// Refund credit after the protocol refund cap and before the EIP-7623 floor.
    pub const fn refund_credit(&self) -> u64 {
        self.refund_credit
    }

    /// EIP-7623 floor gas, or `None` when no floor applies.
    pub const fn floor_gas(&self) -> Option<u64> {
        self.floor_gas
    }

    /// Final receipt gas used: `max(spent_before_refund - refund_credit, floor_gas)`.
    pub const fn gas_used(&self) -> u64 {
        let after_refund = self.spent_before_refund - self.refund_credit;
        match self.floor_gas {
            Some(floor_gas) if floor_gas > after_refund => floor_gas,
            _ => after_refund,
        }
    }
}

/// Actual fee charged for execution gas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionGasFee {
    effective_gas_price: u128,
    charged_amount: U256,
    burnt_amount: U256,
}

impl EvmExecutionGasFee {
    pub(crate) fn new(
        effective_gas_price: u128,
        charged_amount: U256,
        burnt_amount: U256,
    ) -> Result<Self, EvmResultIntegrationError> {
        if burnt_amount > charged_amount {
            return Err(EvmResultIntegrationError::BurntFeeExceedsCharged {
                charged_amount,
                burnt_amount,
            });
        }

        Ok(Self {
            effective_gas_price,
            charged_amount,
            burnt_amount,
        })
    }

    /// Effective per-gas price after applying the block base fee and fee caps.
    pub const fn effective_gas_price(&self) -> u128 {
        self.effective_gas_price
    }

    /// Amount charged for receipt gas used.
    pub const fn charged_amount(&self) -> U256 {
        self.charged_amount
    }

    /// Base-fee portion of the charged amount that is burnt.
    pub const fn burnt_amount(&self) -> U256 {
        self.burnt_amount
    }

    /// Priority-fee portion paid to the block beneficiary.
    pub fn beneficiary_reward(&self) -> U256 {
        self.charged_amount - self.burnt_amount
    }
}

/// Gas and execution-fee facts for a transaction that entered the EVM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionResult {
    gas: EvmGas,
    execution_gas_fee: EvmExecutionGasFee,
}

impl EvmExecutionResult {
    pub(crate) const fn new(gas: EvmGas, execution_gas_fee: EvmExecutionGasFee) -> Self {
        Self {
            gas,
            execution_gas_fee,
        }
    }

    /// Gas accounting for the execution.
    pub const fn gas(&self) -> &EvmGas {
        &self.gas
    }

    /// Actual execution-gas fee settlement.
    pub const fn execution_gas_fee(&self) -> &EvmExecutionGasFee {
        &self.execution_gas_fee
    }
}

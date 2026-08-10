use alloy_primitives::U256;

use super::EspaceResultIntegrationError;

/// Gas accounting returned by the Conflux eSpace executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceGas {
    intrinsic_gas: u64,
    gas_used: u64,
    gas_charged: u64,
}

impl EspaceGas {
    pub(crate) fn new(
        gas_limit: u64,
        intrinsic_gas: u64,
        gas_used: u64,
        gas_charged: u64,
    ) -> Result<Self, EspaceResultIntegrationError> {
        if intrinsic_gas > gas_used || gas_used > gas_limit || gas_charged > gas_limit {
            return Err(EspaceResultIntegrationError::InvalidGasAccounting {
                gas_limit,
                intrinsic_gas,
                gas_used,
                gas_charged,
            });
        }

        Ok(Self {
            intrinsic_gas,
            gas_used,
            gas_charged,
        })
    }

    pub const fn intrinsic_gas(&self) -> u64 {
        self.intrinsic_gas
    }

    pub const fn gas_used(&self) -> u64 {
        self.gas_used
    }

    pub const fn gas_charged(&self) -> u64 {
        self.gas_charged
    }
}

/// Actual eSpace gas-fee settlement for an executed transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceFee {
    charged_amount: U256,
    burnt_amount: Option<U256>,
}

impl EspaceFee {
    pub(crate) fn new(
        charged_amount: U256,
        burnt_amount: Option<U256>,
    ) -> Result<Self, EspaceResultIntegrationError> {
        if let Some(burnt_amount) = burnt_amount
            && burnt_amount > charged_amount
        {
            return Err(EspaceResultIntegrationError::BurntFeeExceedsCharged {
                charged_amount,
                burnt_amount,
            });
        }

        Ok(Self {
            charged_amount,
            burnt_amount,
        })
    }

    pub const fn charged_amount(&self) -> U256 {
        self.charged_amount
    }

    pub const fn burnt_amount(&self) -> Option<U256> {
        self.burnt_amount
    }
}

/// Gas and fee facts for a transaction that entered eSpace execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceExecutionResult {
    gas: EspaceGas,
    fee: EspaceFee,
}

impl EspaceExecutionResult {
    pub(crate) const fn new(gas: EspaceGas, fee: EspaceFee) -> Self {
        Self { gas, fee }
    }

    pub const fn gas(&self) -> &EspaceGas {
        &self.gas
    }

    pub const fn fee(&self) -> &EspaceFee {
        &self.fee
    }
}

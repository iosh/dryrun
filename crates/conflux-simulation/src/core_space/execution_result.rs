use alloy_primitives::U256;

use super::CoreSpaceResultIntegrationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceGas {
    intrinsic_gas: u64,
    gas_used: u64,
    gas_charged: u64,
}

impl CoreSpaceGas {
    pub(crate) fn new(
        gas_limit: U256,
        intrinsic_gas: u64,
        gas_used: u64,
        gas_charged: u64,
    ) -> Result<Self, CoreSpaceResultIntegrationError> {
        if intrinsic_gas > gas_used
            || U256::from(gas_used) > gas_limit
            || U256::from(gas_charged) > gas_limit
        {
            return Err(CoreSpaceResultIntegrationError::InvalidGasAccounting {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceExecutionResult {
    gas: CoreSpaceGas,
    gas_fee: U256,
    burnt_gas_fee: Option<U256>,
    gas_covered_by_sponsor: bool,
    storage_covered_by_sponsor: bool,
}

impl CoreSpaceExecutionResult {
    pub(crate) const fn new(
        gas: CoreSpaceGas,
        gas_fee: U256,
        burnt_gas_fee: Option<U256>,
        gas_covered_by_sponsor: bool,
        storage_covered_by_sponsor: bool,
    ) -> Self {
        Self {
            gas,
            gas_fee,
            burnt_gas_fee,
            gas_covered_by_sponsor,
            storage_covered_by_sponsor,
        }
    }

    pub const fn gas(&self) -> &CoreSpaceGas {
        &self.gas
    }

    pub const fn gas_fee(&self) -> U256 {
        self.gas_fee
    }

    pub const fn burnt_gas_fee(&self) -> Option<U256> {
        self.burnt_gas_fee
    }

    pub const fn gas_covered_by_sponsor(&self) -> bool {
        self.gas_covered_by_sponsor
    }

    pub const fn storage_covered_by_sponsor(&self) -> bool {
        self.storage_covered_by_sponsor
    }
}

mod core_space;
mod espace;

use alloy_primitives::{Address, B256, U256 as AlloyU256};
use cfx_types::{Address as CfxAddress, H256, U256};

use crate::error::ValidationError;

pub(crate) use core_space::SimulateCoreSpaceTransactionRequest;
pub(crate) use espace::SimulateEspaceTransactionRequest;

fn u64_param(value: U256, field: &str) -> Result<u64, ValidationError> {
    u64::try_from(value).map_err(|_| param_exceeds_max(field, value, U256::from(u64::MAX)))
}

fn param_exceeds_max(field: &str, value: U256, max: U256) -> ValidationError {
    ValidationError::invalid_params(format!(
        "`{field}` value {value:#x} exceeds the simulator maximum {max:#x}"
    ))
}

fn cfx_address_to_alloy(address: CfxAddress) -> Address {
    Address::from_slice(address.as_bytes())
}

fn cfx_h256_to_alloy(value: H256) -> B256 {
    B256::from_slice(value.as_bytes())
}

fn cfx_u256_to_alloy(value: U256) -> AlloyU256 {
    let bytes = value.to_big_endian();
    AlloyU256::from_be_bytes(bytes)
}

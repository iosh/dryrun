use cfx_types::U256;

use crate::rpc::error::ValidationError;

pub(super) fn u256_to_u64_quantity(
    value: U256,
    field: &str,
) -> Result<u64, ValidationError> {
    if value > U256::from(u64::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        )));
    }

    Ok(value.as_u64())
}

pub(super) fn u256_to_u32_quantity(
    value: U256,
    field: &str,
) -> Result<u32, ValidationError> {
    if value > U256::from(u32::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 32-bit integer"
        )));
    }

    Ok(value.as_u32())
}


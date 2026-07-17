use crate::errors::ValidationError;

pub(super) fn parse_u64_quantity(value: &str, field: &str) -> Result<u64, ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params(format!("`{field}` must be a 0x-prefixed hex quantity"))
    })?;

    u64::from_str_radix(digits, 16).map_err(|_| {
        ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        ))
    })
}

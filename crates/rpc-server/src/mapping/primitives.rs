use std::str::FromStr;

use alloy_primitives::{Address, B256, Bytes, U256};

use crate::errors::ValidationError;

pub(super) fn parse_address(value: &str, field: &str) -> Result<Address, ValidationError> {
    Address::from_str(value)
        .map_err(|_| ValidationError::invalid_params(format!("`{field}` must be a valid address")))
}

pub(super) fn parse_hash(value: &str, field: &str) -> Result<B256, ValidationError> {
    B256::from_str(value).map_err(|_| {
        ValidationError::invalid_params(format!("`{field}` must be a valid 32-byte hash"))
    })
}

pub(super) fn parse_bytes(value: &str, field: &str) -> Result<Bytes, ValidationError> {
    Bytes::from_str(value)
        .map_err(|_| ValidationError::invalid_params(format!("`{field}` must be valid hex bytes")))
}

pub(super) fn parse_u64_quantity(value: &str, field: &str) -> Result<u64, ValidationError> {
    let digits = strip_hex_prefix(value, field)?;
    u64::from_str_radix(digits, 16).map_err(|_| {
        ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        ))
    })
}

pub(super) fn parse_u128_quantity(value: &str, field: &str) -> Result<u128, ValidationError> {
    let digits = strip_hex_prefix(value, field)?;
    u128::from_str_radix(digits, 16).map_err(|_| {
        ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 128-bit integer"
        ))
    })
}

pub(super) fn parse_u256_quantity(value: &str, field: &str) -> Result<U256, ValidationError> {
    let digits = strip_hex_prefix(value, field)?;
    U256::from_str_radix(digits, 16).map_err(|_| {
        ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 256-bit integer"
        ))
    })
}

pub(super) fn format_u64_quantity(value: u64) -> String {
    format!("0x{value:x}")
}

pub(super) fn format_u256_quantity(value: U256) -> String {
    format!("{value:#x}")
}

fn strip_hex_prefix<'a>(value: &'a str, field: &str) -> Result<&'a str, ValidationError> {
    value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params(format!("`{field}` must be a 0x-prefixed hex quantity"))
    })
}

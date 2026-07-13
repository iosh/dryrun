use std::str::FromStr;

use alloy::primitives::B256;

use crate::errors::ValidationError;

use super::{
    AccessListItem, BlockRef, EvmSimulateTransactionRequest, SimulateTransactionOptions,
    Transaction,
};

impl EvmSimulateTransactionRequest {
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        self.transaction.validate()?;

        if let Some(block) = &self.block {
            block.validate()?;
        }

        if let Some(options) = &self.options {
            options.validate()?;
        }

        Ok(())
    }
}

impl BlockRef {
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Tag(value) => match value.as_str() {
                "latest" => Ok(()),
                "pending" | "safe" | "finalized" => Err(ValidationError::not_supported(
                    "`block` only supports `latest` or a hex block number",
                )),
                value if B256::from_str(value).is_ok() => Err(ValidationError::not_supported(
                    "`block` does not support block hash selectors",
                )),
                value => validate_hex_quantity(value, "block"),
            },
            Self::Hash(_) => Err(ValidationError::not_supported(
                "`block.blockHash` is not supported yet",
            )),
        }
    }
}

impl SimulateTransactionOptions {
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        validate_reserved_option("stateOverrides", self.state_overrides.as_ref())?;
        validate_reserved_option("blockOverrides", self.block_overrides.as_ref())?;
        validate_reserved_option("include", self.include.as_ref())?;

        Ok(())
    }
}

impl Transaction {
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        if let Some(tx_type) = self.tx_type
            && !matches!(tx_type, 0x0..=0x2)
        {
            return Err(ValidationError::not_supported(
                "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
            ));
        }

        if let Some(access_list) = &self.access_list {
            for (index, entry) in access_list.iter().enumerate() {
                entry.validate(index)?;
            }
        }

        let has_gas_price = self.gas_price.is_some();
        let has_dynamic_fee =
            self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some();

        if has_gas_price && has_dynamic_fee {
            return Err(ValidationError::invalid_params(
                "`transaction.gasPrice` cannot be mixed with EIP-1559 fee fields",
            ));
        }

        match self.tx_type {
            Some(0x0) => {
                if self
                    .access_list
                    .as_ref()
                    .is_some_and(|items| !items.is_empty())
                {
                    return Err(ValidationError::invalid_params(
                        "`transaction.type` `0x0` cannot be combined with `transaction.accessList`",
                    ));
                }
                if has_dynamic_fee {
                    return Err(ValidationError::invalid_params(
                        "`transaction.type` `0x0` cannot be combined with EIP-1559 fee fields",
                    ));
                }
            }
            Some(0x1) => {
                if has_dynamic_fee {
                    return Err(ValidationError::invalid_params(
                        "`transaction.type` `0x1` cannot be combined with EIP-1559 fee fields",
                    ));
                }
            }
            Some(0x2) => {
                if has_gas_price {
                    return Err(ValidationError::invalid_params(
                        "`transaction.type` `0x2` cannot be combined with `transaction.gasPrice`",
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl AccessListItem {
    pub(crate) fn validate(&self, _index: usize) -> Result<(), ValidationError> {
        Ok(())
    }
}

fn validate_reserved_option(
    field: &str,
    value: Option<&serde_json::Value>,
) -> Result<(), ValidationError> {
    if value.is_some() {
        return Err(ValidationError::not_supported(format!(
            "`options.{field}` is reserved and not supported yet"
        )));
    }

    Ok(())
}

fn validate_hex_quantity(value: &str, field: &str) -> Result<(), ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params(format!("`{field}` must be a 0x-prefixed hex quantity"))
    })?;

    if digits.is_empty() {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must not be empty"
        )));
    }

    if !digits.chars().all(|char| char.is_ascii_hexdigit()) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must be a hex quantity"
        )));
    }

    if digits.len() > 1 && digits.starts_with('0') {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must not contain leading zeroes"
        )));
    }

    Ok(())
}

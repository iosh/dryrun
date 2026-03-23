use std::str::FromStr;

use alloy::primitives::{Address, B256};

use crate::errors::ValidationError;

use super::{AccessListItem, BlockRef, EvmSimulateTransactionRequest, SimulationOptions, Transaction};

impl EvmSimulateTransactionRequest {
    pub(crate) fn validate(&self) -> Result<(), ValidationError> {
        if let Some(block) = &self.block {
            block.validate()?;
        }

        if let Some(options) = &self.options {
            options.validate()?;
        }

        self.transaction.validate()
    }
}

impl BlockRef {
    fn validate(&self) -> Result<(), ValidationError> {
        let provided = usize::from(self.tag.is_some())
            + usize::from(self.number.is_some())
            + usize::from(self.hash.is_some());
        if provided != 1 {
            return Err(ValidationError::invalid_params(
                "`block` must contain exactly one of `tag`, `number`, or `hash`",
            ));
        }

        if let Some(tag) = &self.tag {
            if tag != "latest" {
                return Err(ValidationError::invalid_params(
                    "`block.tag` only supports `latest` in v0",
                ));
            }
        }

        if let Some(number) = &self.number {
            validate_hex_quantity(number, "block.number")?;
        }

        if let Some(hash) = &self.hash {
            validate_b256(hash, "block.hash")?;
        }

        Ok(())
    }
}

impl SimulationOptions {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.include_trace.unwrap_or(false) {
            return Err(ValidationError::not_supported(
                "`options.includeTrace` is not supported in v0",
            ));
        }

        if self.include_state_changes.unwrap_or(false) {
            return Err(ValidationError::not_supported(
                "`options.includeStateChanges` is not supported in v0",
            ));
        }

        Ok(())
    }
}

impl Transaction {
    fn validate(&self) -> Result<(), ValidationError> {
        match self.tx_type.as_str() {
            "0x0" | "0x1" | "0x2" => {}
            _ => {
                return Err(ValidationError::not_supported(
                    "`transaction.type` only supports `0x0`, `0x1`, and `0x2` in v0",
                ));
            }
        }

        if self.blob_versioned_hashes.is_some()
            || self.max_fee_per_blob_gas.is_some()
            || self.sidecar.is_some()
            || self.authorization_list.is_some()
        {
            return Err(ValidationError::not_supported(
                "blob or authorization fields are not supported in v0",
            ));
        }

        validate_hex_quantity(&self.chain_id, "transaction.chainId")?;
        validate_address(&self.from, "transaction.from")?;

        if let Some(to) = &self.to {
            validate_address(to, "transaction.to")?;
        }

        validate_hex_quantity(&self.nonce, "transaction.nonce")?;
        validate_hex_quantity(&self.gas, "transaction.gas")?;
        validate_hex_quantity(&self.value, "transaction.value")?;
        validate_hex_bytes(&self.data, "transaction.data")?;

        if let Some(access_list) = &self.access_list {
            for (idx, entry) in access_list.iter().enumerate() {
                entry.validate(idx)?;
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

        match self.tx_type.as_str() {
            "0x0" | "0x1" => {
                let gas_price = self.gas_price.as_deref().ok_or_else(|| {
                    ValidationError::invalid_params(
                        "`transaction.gasPrice` is required for type `0x0` and `0x1`",
                    )
                })?;
                validate_hex_quantity(gas_price, "transaction.gasPrice")?;
            }
            "0x2" => {
                let max_fee = self.max_fee_per_gas.as_deref().ok_or_else(|| {
                    ValidationError::invalid_params(
                        "`transaction.maxFeePerGas` is required for type `0x2`",
                    )
                })?;
                let max_priority = self.max_priority_fee_per_gas.as_deref().ok_or_else(|| {
                    ValidationError::invalid_params(
                        "`transaction.maxPriorityFeePerGas` is required for type `0x2`",
                    )
                })?;
                validate_hex_quantity(max_fee, "transaction.maxFeePerGas")?;
                validate_hex_quantity(max_priority, "transaction.maxPriorityFeePerGas")?;
            }
            _ => {}
        }

        Ok(())
    }
}

impl AccessListItem {
    fn validate(&self, index: usize) -> Result<(), ValidationError> {
        validate_address(
            &self.address,
            &format!("transaction.accessList[{index}].address"),
        )?;
        for (slot_index, slot) in self.storage_keys.iter().enumerate() {
            validate_b256(
                slot,
                &format!("transaction.accessList[{index}].storageKeys[{slot_index}]"),
            )?;
        }
        Ok(())
    }
}

fn validate_address(value: &str, field: &str) -> Result<(), ValidationError> {
    Address::from_str(value).map(|_| ()).map_err(|_| {
        ValidationError::invalid_params(format!("`{field}` must be a valid address"))
    })
}

fn validate_b256(value: &str, field: &str) -> Result<(), ValidationError> {
    B256::from_str(value).map(|_| ()).map_err(|_| {
        ValidationError::invalid_params(format!("`{field}` must be a valid 32-byte hash"))
    })
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

fn validate_hex_bytes(value: &str, field: &str) -> Result<(), ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params(format!("`{field}` must be 0x-prefixed bytes"))
    })?;

    if digits.len() % 2 != 0 {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must contain an even number of hex characters"
        )));
    }

    if !digits.chars().all(|char| char.is_ascii_hexdigit()) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must be valid hex bytes"
        )));
    }

    Ok(())
}

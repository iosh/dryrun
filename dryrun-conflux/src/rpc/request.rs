use std::str::FromStr;

use cfx_bytes::Bytes;
use cfx_rpc_eth_types::TransactionRequest;
use cfx_types::{Address, H256, U256};
use conflux_service::espace as service_espace;
use serde::Deserialize;
use serde_json::Value;

use super::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SimulateEspaceTransactionRequest {
    transaction: TransactionRequest,
    #[serde(default)]
    block: Option<BlockRef>,
    #[serde(default)]
    options: Option<SimulateTransactionOptions>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum BlockRef {
    Tag(String),
    Hash(BlockHashRef),
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlockHashRef {
    block_hash: H256,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SimulateTransactionOptions {
    #[serde(default)]
    state_overrides: Option<Value>,
    #[serde(default)]
    block_overrides: Option<Value>,
    #[serde(default)]
    include: Option<Value>,
}

impl TryFrom<SimulateEspaceTransactionRequest> for service_espace::SimulateEspaceTransactionInput {
    type Error = ValidationError;

    fn try_from(request: SimulateEspaceTransactionRequest) -> Result<Self, Self::Error> {
        request.validate()?;

        Ok(Self {
            block: request
                .block
                .map(map_block_ref)
                .transpose()?
                .unwrap_or(service_espace::BlockRef::Latest),
            transaction: map_transaction(request.transaction)?,
        })
    }
}

impl SimulateEspaceTransactionRequest {
    fn validate(&self) -> Result<(), ValidationError> {
        validate_transaction(&self.transaction)?;

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
    fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Tag(value) => match value.as_str() {
                "latest" => Ok(()),
                "pending" | "safe" | "finalized" => Err(ValidationError::not_supported(
                    "`block` only supports `latest` or a hex block number",
                )),
                value if H256::from_str(value).is_ok() => Err(ValidationError::not_supported(
                    "`block` does not support block hash selectors yet",
                )),
                value => parse_u64_quantity(value, "block").map(|_| ()),
            },
            Self::Hash(_) => Err(ValidationError::not_supported(
                "`block.blockHash` is not supported yet",
            )),
        }
    }
}

impl SimulateTransactionOptions {
    fn validate(&self) -> Result<(), ValidationError> {
        validate_reserved_option("stateOverrides", self.state_overrides.as_ref())?;
        validate_reserved_option("blockOverrides", self.block_overrides.as_ref())?;
        validate_reserved_option("include", self.include.as_ref())?;

        Ok(())
    }
}

fn validate_transaction(transaction: &TransactionRequest) -> Result<(), ValidationError> {
    if transaction.from.is_none() {
        return Err(ValidationError::invalid_params(
            "`transaction.from` is required",
        ));
    }

    if transaction.gas.is_none() {
        return Err(ValidationError::invalid_params(
            "`transaction.gas` is required",
        ));
    }

    if transaction.authorization_list.is_some() {
        return Err(ValidationError::not_supported(
            "`transaction.authorizationList` is not supported yet",
        ));
    }

    if let Some(tx_type) = transaction.transaction_type
        && tx_type.as_u64() > 0x2
    {
        if tx_type.as_u64() == 0x4 {
            return Err(ValidationError::not_supported(
                "`transaction.type` `0x4` / EIP-7702 is not supported yet",
            ));
        }

        return Err(ValidationError::not_supported(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        ));
    }

    let has_gas_price = transaction.gas_price.is_some();
    let has_dynamic_fee =
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some();

    if has_gas_price && has_dynamic_fee {
        return Err(ValidationError::invalid_params(
            "`transaction.gasPrice` cannot be mixed with EIP-1559 fee fields",
        ));
    }

    if let (Some(max_fee), Some(max_priority)) = (
        transaction.max_fee_per_gas,
        transaction.max_priority_fee_per_gas,
    ) && max_priority > max_fee
    {
        return Err(ValidationError::invalid_params(
            "`transaction.maxPriorityFeePerGas` cannot exceed `transaction.maxFeePerGas`",
        ));
    }

    match transaction.transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => {
            if transaction
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

fn require_transaction_from(transaction: &TransactionRequest) -> Result<Address, ValidationError> {
    transaction
        .from
        .ok_or_else(|| ValidationError::invalid_params("`transaction.from` is required"))
}

fn require_transaction_gas(transaction: &TransactionRequest) -> Result<U256, ValidationError> {
    transaction
        .gas
        .ok_or_else(|| ValidationError::invalid_params("`transaction.gas` is required"))
}

fn u256_to_u64_quantity(value: U256, field: &str) -> Result<u64, ValidationError> {
    if value > U256::from(u64::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        )));
    }

    Ok(value.as_u64())
}

fn validate_reserved_option(field: &str, value: Option<&Value>) -> Result<(), ValidationError> {
    if value.is_some() {
        return Err(ValidationError::not_supported(format!(
            "`options.{field}` is reserved and not supported yet"
        )));
    }

    Ok(())
}

fn map_block_ref(block: BlockRef) -> Result<service_espace::BlockRef, ValidationError> {
    match block {
        BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(service_espace::BlockRef::Latest),
            value => Ok(service_espace::BlockRef::Number(parse_u64_quantity(
                value, "block",
            )?)),
        },
        BlockRef::Hash(selector) => Ok(service_espace::BlockRef::Hash(selector.block_hash)),
    }
}

fn map_transaction(
    transaction: TransactionRequest,
) -> Result<service_espace::EspaceTransaction, ValidationError> {
    let tx_type = infer_transaction_type(&transaction)?;
    let from = require_transaction_from(&transaction)?;
    let gas_limit = require_transaction_gas(&transaction)?;

    let TransactionRequest {
        to,
        nonce,
        value,
        input,
        access_list,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        chain_id,
        ..
    } = transaction;

    let data: Bytes = input
        .try_into_unique_input()
        .map_err(|error| {
            ValidationError::invalid_params(format!("`transaction.input` is invalid: {error}"))
        })?
        .unwrap_or_default()
        .into();

    Ok(service_espace::EspaceTransaction {
        tx_type,
        requested_chain_id: chain_id
            .map(|value| u256_to_u64_quantity(value, "transaction.chainId"))
            .transpose()?,
        from,
        to,
        nonce,
        gas_limit,
        value: value.unwrap_or_default(),
        data,
        access_list: access_list
            .unwrap_or_default()
            .into_iter()
            .map(|item| service_espace::AccessListItem {
                address: item.address,
                storage_keys: item.storage_keys,
            })
            .collect(),
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
    })
}

fn infer_transaction_type(
    transaction: &TransactionRequest,
) -> Result<service_espace::EspaceTransactionType, ValidationError> {
    Ok(
        match transaction.transaction_type.map(|value| value.as_u64()) {
            Some(0x0) => service_espace::EspaceTransactionType::Legacy,
            Some(0x1) => service_espace::EspaceTransactionType::AccessList,
            Some(0x2) => service_espace::EspaceTransactionType::DynamicFee,
            None if transaction.max_fee_per_gas.is_some()
                || transaction.max_priority_fee_per_gas.is_some() =>
            {
                service_espace::EspaceTransactionType::DynamicFee
            }
            None if transaction
                .access_list
                .as_ref()
                .is_some_and(|items| !items.is_empty()) =>
            {
                service_espace::EspaceTransactionType::AccessList
            }
            None => service_espace::EspaceTransactionType::Legacy,
            Some(0x4) => {
                return Err(ValidationError::not_supported(
                    "`transaction.type` `0x4` / EIP-7702 is not supported yet",
                ));
            }
            Some(_) => {
                return Err(ValidationError::not_supported(
                    "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
                ));
            }
        },
    )
}

fn parse_u64_quantity(value: &str, field: &str) -> Result<u64, ValidationError> {
    let value = parse_quantity(value)?;

    if value > U256::from(u64::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        )));
    }

    Ok(value.as_u64())
}

fn parse_quantity(value: &str) -> Result<U256, ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params("quantity must be a 0x-prefixed hex string")
    })?;

    if digits.is_empty() {
        return Err(ValidationError::invalid_params(
            "quantity must not be empty",
        ));
    }

    if digits.len() > 1 && digits.starts_with('0') {
        return Err(ValidationError::invalid_params(
            "quantity must not contain leading zeroes",
        ));
    }

    let mut normalized = digits.to_string();
    if normalized.len() % 2 == 1 {
        normalized.insert(0, '0');
    }

    let bytes = hex::decode(&normalized)
        .map_err(|_| ValidationError::invalid_params("quantity must be a hex string"))?;

    if bytes.len() > 32 {
        return Err(ValidationError::invalid_params(
            "quantity must fit into an unsigned 256-bit integer",
        ));
    }

    Ok(U256::from_big_endian(&bytes))
}

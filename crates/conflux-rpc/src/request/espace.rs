use std::str::FromStr;

use cfx_rpc_eth_types::TransactionRequest;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::espace as service_espace;
use serde::Deserialize;
use serde_json::Value;
use simulation_transaction::{
    AccessListItem as SimulationAccessListItem, TransactionRequest as SimulationTransactionRequest,
    TransactionType,
};

use super::primitives::{to_alloy_address, to_alloy_b256, to_alloy_u256};
use crate::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SimulateEspaceTransactionRequest {
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
                .unwrap_or(service_espace::EspaceBlockRef::Latest),
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
    if transaction.authorization_list.is_some() {
        return Err(ValidationError::not_supported(
            "`transaction.authorizationList` is not supported yet",
        ));
    }

    Ok(())
}
fn require_transaction_from(transaction: &TransactionRequest) -> Result<Address, ValidationError> {
    transaction
        .from
        .ok_or_else(|| ValidationError::invalid_params("`transaction.from` is required"))
}

fn validate_reserved_option(field: &str, value: Option<&Value>) -> Result<(), ValidationError> {
    if value.is_some() {
        return Err(ValidationError::not_supported(format!(
            "`options.{field}` is reserved and not supported yet"
        )));
    }

    Ok(())
}

fn map_block_ref(block: BlockRef) -> Result<service_espace::EspaceBlockRef, ValidationError> {
    match block {
        BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(service_espace::EspaceBlockRef::Latest),
            value => Ok(service_espace::EspaceBlockRef::Number(parse_u64_quantity(
                value, "block",
            )?)),
        },
        BlockRef::Hash(_) => Err(ValidationError::not_supported(
            "eSpace block hash selectors are not supported yet",
        )),
    }
}
fn map_transaction(
    transaction: TransactionRequest,
) -> Result<SimulationTransactionRequest, ValidationError> {
    let transaction_type = to_transaction_type(transaction.transaction_type)?;
    let tx_type = resolve_transaction_type(&transaction, transaction_type);
    validate_transaction_shape(&transaction, tx_type)?;

    let from = require_transaction_from(&transaction)?;

    let TransactionRequest {
        to,
        nonce,
        gas,
        value,
        input,
        access_list,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        chain_id,
        ..
    } = transaction;

    let input = input.try_into_unique_input().map_err(|error| {
        ValidationError::invalid_params(format!("`transaction.input` is invalid: {error}"))
    })?;

    let request = SimulationTransactionRequest {
        from: to_alloy_address(from),
        to: to.map(to_alloy_address),
        nonce: nonce.map(to_alloy_u256),
        gas_limit: gas.map(to_alloy_u256),
        value: value.map(to_alloy_u256),
        input,
        chain_id: chain_id.map(to_alloy_u256),
        transaction_type,
        access_list: access_list.map(|items| {
            items
                .into_iter()
                .map(|item| SimulationAccessListItem {
                    address: to_alloy_address(item.address),
                    storage_keys: item.storage_keys.into_iter().map(to_alloy_b256).collect(),
                })
                .collect()
        }),
        gas_price: gas_price.map(to_alloy_u256),
        max_fee_per_gas: max_fee_per_gas.map(to_alloy_u256),
        max_priority_fee_per_gas: max_priority_fee_per_gas.map(to_alloy_u256),
    };

    Ok(request)
}

fn validate_transaction_shape(
    transaction: &TransactionRequest,
    tx_type: TransactionType,
) -> Result<(), ValidationError> {
    let has_dynamic_fee =
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some();

    match tx_type {
        TransactionType::Legacy => {
            if transaction.access_list.is_some() {
                return Err(ValidationError::invalid_params(
                    "legacy transactions cannot include `transaction.accessList`",
                ));
            }

            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "legacy transactions cannot include EIP-1559 fee fields",
                ));
            }
        }
        TransactionType::AccessList => {
            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "EIP-2930 transactions cannot include EIP-1559 fee fields",
                ));
            }
        }
        TransactionType::DynamicFee => {
            if transaction.gas_price.is_some() {
                return Err(ValidationError::invalid_params(
                    "EIP-1559 transactions cannot include `transaction.gasPrice`",
                ));
            }
        }
    }

    Ok(())
}

fn resolve_transaction_type(
    transaction: &TransactionRequest,
    transaction_type: Option<TransactionType>,
) -> TransactionType {
    match transaction_type {
        Some(transaction_type) => transaction_type,
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            TransactionType::DynamicFee
        }
        None if transaction.access_list.is_some() => TransactionType::AccessList,
        None => TransactionType::Legacy,
    }
}

fn to_transaction_type(
    transaction_type: Option<U64>,
) -> Result<Option<TransactionType>, ValidationError> {
    match transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => Ok(Some(TransactionType::Legacy)),
        Some(0x1) => Ok(Some(TransactionType::AccessList)),
        Some(0x2) => Ok(Some(TransactionType::DynamicFee)),
        Some(0x4) => Err(ValidationError::not_supported(
            "`transaction.type` `0x4` / EIP-7702 is not supported yet",
        )),
        Some(_) => Err(ValidationError::invalid_params(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        )),
        None => Ok(None),
    }
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

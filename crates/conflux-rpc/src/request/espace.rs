use std::str::FromStr;

use alloy_primitives::Bytes;
use cfx_rpc_eth_types::TransactionRequest;
use cfx_types::{Address as CfxAddress, H256, U64, U256};
use conflux_service::espace as service_espace;
use serde::Deserialize;
use serde_json::Value;
use simulation_transaction::{TransactionType, TransactionVariantRequest};

use super::{cfx_address_to_alloy, cfx_h256_to_alloy, cfx_u256_to_alloy, u64_param, u128_param};
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

impl TryFrom<SimulateEspaceTransactionRequest> for service_espace::EspaceSimulationInput {
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
                value => parse_u64_param(value, "block").map(|_| ()),
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

fn require_transaction_from(
    transaction: &TransactionRequest,
) -> Result<CfxAddress, ValidationError> {
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
            value => Ok(service_espace::EspaceBlockRef::Number(parse_u64_param(
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
) -> Result<service_espace::EspaceTransactionRequest, ValidationError> {
    let transaction_type = map_transaction_type(transaction.transaction_type)?;
    let transaction_type = TransactionType::infer(
        transaction_type,
        transaction.access_list.is_some(),
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some(),
    );
    validate_legacy_access_list(&transaction, transaction_type)?;
    let from = require_transaction_from(&transaction)?;
    let chain_id = transaction
        .chain_id
        .ok_or_else(|| ValidationError::invalid_params("`transaction.chainId` is required"))?;

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
        ..
    } = transaction;

    let input = input
        .try_into_unique_input()
        .map_err(|error| {
            ValidationError::invalid_params(format!("`transaction.input` is invalid: {error}"))
        })?
        .map(|input| Bytes::from(input.to_vec()));
    let variant = TransactionVariantRequest::try_new(
        transaction_type,
        access_list.map(|items| {
            items
                .into_iter()
                .map(|item| simulation_transaction::AccessListItem {
                    address: cfx_address_to_alloy(item.address),
                    storage_keys: item
                        .storage_keys
                        .into_iter()
                        .map(cfx_h256_to_alloy)
                        .collect(),
                })
                .collect()
        }),
        gas_price
            .map(|value| u128_param(value, "transaction.gasPrice"))
            .transpose()?,
        max_fee_per_gas
            .map(|value| u128_param(value, "transaction.maxFeePerGas"))
            .transpose()?,
        max_priority_fee_per_gas
            .map(|value| u128_param(value, "transaction.maxPriorityFeePerGas"))
            .transpose()?,
    )
    .map_err(|error| ValidationError::invalid_params(error.to_string()))?;

    Ok(service_espace::EspaceTransactionRequest {
        from: cfx_address_to_alloy(from),
        to: to.map(cfx_address_to_alloy),
        nonce: nonce
            .map(|value| u64_param(value, "transaction.nonce"))
            .transpose()?,
        gas_limit: gas
            .map(|value| u64_param(value, "transaction.gas"))
            .transpose()?,
        value: value.map(cfx_u256_to_alloy),
        data: input,
        chain_id: u64_param(chain_id, "transaction.chainId")?,
        variant,
    })
}

fn validate_legacy_access_list(
    transaction: &TransactionRequest,
    transaction_type: TransactionType,
) -> Result<(), ValidationError> {
    if transaction_type == TransactionType::Legacy && transaction.access_list.is_some() {
        return Err(ValidationError::invalid_params(
            "legacy transactions cannot include `transaction.accessList`",
        ));
    }

    Ok(())
}

fn map_transaction_type(
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

fn parse_u64_param(value: &str, field: &str) -> Result<u64, ValidationError> {
    u64_param(parse_hex_param(value, field)?, field)
}

fn parse_hex_param(value: &str, field: &str) -> Result<U256, ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params(format!("`{field}` must be a 0x-prefixed hex string"))
    })?;

    if digits.is_empty() {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must not be empty"
        )));
    }

    if digits.len() > 1 && digits.starts_with('0') {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must not contain leading zeroes"
        )));
    }

    let mut normalized = digits.to_string();
    if normalized.len() % 2 == 1 {
        normalized.insert(0, '0');
    }

    let bytes = hex::decode(&normalized)
        .map_err(|_| ValidationError::invalid_params(format!("`{field}` must be a hex string")))?;

    if bytes.len() > 32 {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 256-bit integer"
        )));
    }

    Ok(U256::from_big_endian(&bytes))
}

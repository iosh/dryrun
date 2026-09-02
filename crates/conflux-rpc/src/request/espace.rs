use std::str::FromStr;

use alloy_primitives::Bytes;
use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_types::{Address as CfxAddress, H256, U64, U256};
use conflux_simulation::espace::{
    AccessListItem, Authorization, EspaceBlockSelector, EspacePartialTransaction,
    EspaceSimulationRequest, EspaceTransactionInput, SignedAuthorization, TxType,
};
use serde::Deserialize;
use serde_json::Value;

use super::{cfx_address_to_alloy, cfx_h256_to_alloy, cfx_u256_to_alloy, u64_param};
use crate::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EspaceRpcTransactionRequest {
    from: Option<CfxAddress>,
    to: Option<CfxAddress>,
    gas_price: Option<U256>,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
    gas: Option<U256>,
    value: Option<U256>,
    input: Option<RpcBytes>,
    data: Option<RpcBytes>,
    nonce: Option<U256>,
    access_list: Option<Vec<RpcAccessListItem>>,
    #[serde(rename = "type")]
    transaction_type: Option<U64>,
    chain_id: Option<U256>,
    max_fee_per_blob_gas: Option<U256>,
    blob_versioned_hashes: Option<Vec<H256>>,
    authorization_list: Option<Vec<RpcSignedAuthorization>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RpcAccessListItem {
    address: CfxAddress,
    storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RpcSignedAuthorization {
    chain_id: U256,
    address: CfxAddress,
    nonce: U64,
    y_parity: U64,
    r: U256,
    s: U256,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SimulateEspaceTransactionRequest {
    transaction: EspaceRpcTransactionRequest,
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

impl TryFrom<SimulateEspaceTransactionRequest> for EspaceSimulationRequest {
    type Error = ValidationError;

    fn try_from(request: SimulateEspaceTransactionRequest) -> Result<Self, Self::Error> {
        request.validate()?;

        Ok(Self {
            block: request
                .block
                .map(map_block_ref)
                .transpose()?
                .unwrap_or(EspaceBlockSelector::Latest),
            transaction: map_transaction(request.transaction)?,
        })
    }
}

impl SimulateEspaceTransactionRequest {
    fn validate(&self) -> Result<(), ValidationError> {
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
                "pending" | "earliest" | "safe" | "finalized" => {
                    Err(ValidationError::not_supported(
                        "`block` supports `latest`, a hex block number, or a block hash",
                    ))
                }
                value if H256::from_str(value).is_ok() => Ok(()),
                value => parse_u64_param(value, "block").map(|_| ()),
            },
            Self::Hash(_) => Ok(()),
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

fn require_transaction_from(
    transaction: &EspaceRpcTransactionRequest,
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

fn map_block_ref(block: BlockRef) -> Result<EspaceBlockSelector, ValidationError> {
    match block {
        BlockRef::Tag(value) if value == "latest" => Ok(EspaceBlockSelector::Latest),
        BlockRef::Tag(value) => H256::from_str(&value)
            .map(|hash| EspaceBlockSelector::Hash(cfx_h256_to_alloy(hash)))
            .or_else(|_| parse_u64_param(&value, "block").map(EspaceBlockSelector::Number)),
        BlockRef::Hash(block) => Ok(EspaceBlockSelector::Hash(cfx_h256_to_alloy(
            block.block_hash,
        ))),
    }
}

fn map_transaction(
    transaction: EspaceRpcTransactionRequest,
) -> Result<EspaceTransactionInput, ValidationError> {
    let from = require_transaction_from(&transaction)?;
    let EspaceRpcTransactionRequest {
        to,
        nonce,
        gas,
        value,
        input,
        data,
        access_list,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        max_fee_per_blob_gas,
        blob_versioned_hashes,
        authorization_list,
        transaction_type,
        chain_id,
        ..
    } = transaction;
    let chain_id = chain_id
        .ok_or_else(|| ValidationError::invalid_params("`transaction.chainId` is required"))?;
    let chain_id = u64_param(chain_id, "transaction.chainId")?;
    let input = match (input, data) {
        (Some(input), Some(data)) if input != data => {
            return Err(ValidationError::invalid_params(
                "`transaction.input` and `transaction.data` must be equal when both are provided",
            ));
        }
        (Some(input), _) | (_, Some(input)) => Some(Bytes::from(input.0)),
        (None, None) => None,
    };
    let transaction_type = transaction_type
        .map(|value| match value.as_u64() {
            0x3 => Err(ValidationError::not_supported(
                "`transaction.type` `0x3` / EIP-4844 is not supported by eSpace",
            )),
            raw => TxType::try_from(raw).map_err(|_| {
                ValidationError::invalid_params(
                    "`transaction.type` must be one of `0x0`, `0x1`, `0x2`, or `0x4`",
                )
            }),
        })
        .transpose()?;
    let authorization_list = authorization_list
        .map(|items| items.into_iter().map(map_signed_authorization).collect())
        .transpose()?;

    Ok(EspaceTransactionInput::Partial(EspacePartialTransaction {
        from: cfx_address_to_alloy(from),
        to: to.map(cfx_address_to_alloy),
        nonce: nonce
            .map(|value| u64_param(value, "transaction.nonce"))
            .transpose()?,
        gas_limit: gas
            .map(|value| u64_param(value, "transaction.gas"))
            .transpose()?,
        value: value.map(cfx_u256_to_alloy),
        input,
        chain_id: Some(chain_id),
        transaction_type,
        gas_price: gas_price.map(cfx_u256_to_alloy),
        max_fee_per_gas: max_fee_per_gas.map(cfx_u256_to_alloy),
        max_priority_fee_per_gas: max_priority_fee_per_gas.map(cfx_u256_to_alloy),
        max_fee_per_blob_gas: max_fee_per_blob_gas.map(cfx_u256_to_alloy),
        access_list: map_access_list(access_list),
        blob_versioned_hashes: blob_versioned_hashes
            .map(|items| items.into_iter().map(cfx_h256_to_alloy).collect()),
        authorization_list,
    }))
}

fn map_access_list(items: Option<Vec<RpcAccessListItem>>) -> Option<Vec<AccessListItem>> {
    items.map(|items| {
        items
            .into_iter()
            .map(|item| AccessListItem {
                address: cfx_address_to_alloy(item.address),
                storage_keys: item
                    .storage_keys
                    .into_iter()
                    .map(cfx_h256_to_alloy)
                    .collect(),
            })
            .collect()
    })
}

fn map_signed_authorization(
    authorization: RpcSignedAuthorization,
) -> Result<SignedAuthorization, ValidationError> {
    let y_parity = u8::try_from(authorization.y_parity.as_u64()).map_err(|_| {
        ValidationError::invalid_params(
            "`transaction.authorizationList[].yParity` must fit into an unsigned 8-bit integer",
        )
    })?;
    Ok(SignedAuthorization::new_unchecked(
        Authorization {
            chain_id: cfx_u256_to_alloy(authorization.chain_id),
            address: cfx_address_to_alloy(authorization.address),
            nonce: authorization.nonce.as_u64(),
        },
        y_parity,
        cfx_u256_to_alloy(authorization.r),
        cfx_u256_to_alloy(authorization.s),
    ))
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
    let mut normalized = digits.to_owned();
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

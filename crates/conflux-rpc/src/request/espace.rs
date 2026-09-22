use std::str::FromStr;

use cfx_types::{H256, U256};
use conflux_simulation::espace::{
    EspaceBlockSelector, EspaceSimulationRequest, EspaceTransactionInput, EspaceTransactionRequest,
};
use serde::Deserialize;
use serde_json::Value;

use super::{cfx_h256_to_alloy, u64_param};
use crate::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SimulateEspaceTransactionRequest {
    transaction: EspaceTransactionRequest,
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
            transaction: EspaceTransactionInput::Partial(request.transaction),
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

use alloy::{eips::BlockId, rpc::types::TransactionRequest};
use jsonrpsee::types::ErrorObjectOwned;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use simulation_core::ErrorCode;

/// Parameters of the block-based simulation methods.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BlockRequest {
    transaction: TransactionRequest,
    #[serde(default)]
    block: Option<BlockId>,
    /// Reserved for state and block overrides.
    #[serde(default)]
    options: Option<Value>,
}

impl Request for BlockRequest {
    type Selector = BlockId;
    type Transaction = TransactionRequest;
    fn into_parts(self) -> Result<(BlockId, TransactionRequest), ErrorObjectOwned> {
        if self.options.is_some() {
            return Err(super::error::rpc_error_object(
                ErrorCode::Unsupported,
                "options are not supported".into(),
            ));
        }
        Ok((self.block.unwrap_or_default(), self.transaction))
    }
}

pub(super) trait Request: DeserializeOwned + Send + 'static {
    type Selector: Send + 'static;
    type Transaction: Send + 'static;
    fn into_parts(self) -> Result<(Self::Selector, Self::Transaction), ErrorObjectOwned>;
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EpochRequest {
    #[serde(deserialize_with = "core_transaction")]
    transaction: conflux_simulation::core_space::TransactionRequest,
    #[serde(default)]
    epoch: Option<conflux_provider::EpochNumber>,
    #[serde(default)]
    options: Option<Value>,
}

impl Request for EpochRequest {
    type Selector = conflux_provider::EpochNumber;
    type Transaction = conflux_simulation::core_space::TransactionRequest;
    fn into_parts(self) -> Result<(Self::Selector, Self::Transaction), ErrorObjectOwned> {
        if self.options.is_some() {
            return Err(super::error::rpc_error_object(
                ErrorCode::Unsupported,
                "options are not supported".into(),
            ));
        }
        Ok((
            self.epoch
                .unwrap_or(conflux_provider::EpochNumber::LatestState),
            self.transaction,
        ))
    }
}

fn core_transaction<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<conflux_simulation::core_space::TransactionRequest, D::Error> {
    let mut unknown = Vec::new();
    let request = serde_ignored::deserialize(deserializer, |path| unknown.push(path.to_string()))?;
    if !unknown.is_empty() {
        return Err(serde::de::Error::custom(format!(
            "unknown transaction fields: {}",
            unknown.join(", ")
        )));
    }
    Ok(request)
}

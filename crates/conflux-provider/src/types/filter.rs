use alloy_primitives::{B128, B256, Bytes, U256};
use serde::{Deserialize, Deserializer, Serialize};

use crate::CoreAddress;

use super::{common::EpochNumber, core::CoreSpace};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreLogFilter {
    pub from_epoch: Option<EpochNumber>,
    pub to_epoch: Option<EpochNumber>,
    pub from_block: Option<U256>,
    pub to_block: Option<U256>,
    pub block_hashes: Option<Vec<B256>>,
    pub address: Option<CoreVariadic<CoreAddress>>,
    pub topics: Option<Vec<Option<CoreVariadic<B256>>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CoreVariadic<T> {
    Single(T),
    Multiple(Vec<T>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CoreFilterId(pub B128);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreFilterLog {
    pub address: CoreAddress,
    pub topics: Vec<B256>,
    pub data: Bytes,
    pub block_hash: Option<B256>,
    pub epoch_number: Option<U256>,
    pub transaction_hash: Option<B256>,
    pub transaction_index: Option<U256>,
    pub log_index: Option<U256>,
    pub transaction_log_index: Option<U256>,
    pub space: Option<CoreSpace>,
    pub block_timestamp: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreFilterChangeReorg {
    pub revert_to: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum CoreFilterChange {
    Log(CoreFilterLog),
    ChainReorg(CoreFilterChangeReorg),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreFilterChanges {
    Logs(Vec<CoreFilterChange>),
    Hashes(Vec<B256>),
    Empty,
}

impl<'de> Deserialize<'de> for CoreFilterChanges {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Vec::<serde_json::Value>::deserialize(deserializer)?;
        if value.is_empty() {
            return Ok(Self::Empty);
        }
        if value.first().is_some_and(serde_json::Value::is_string) {
            return serde_json::from_value(serde_json::Value::Array(value))
                .map(Self::Hashes)
                .map_err(|error| serde::de::Error::custom(error.to_string()));
        }
        let mut logs = Vec::with_capacity(value.len());
        for item in value {
            if item.get("revertTo").is_some() {
                logs.push(CoreFilterChange::ChainReorg(
                    serde_json::from_value(item)
                        .map_err(|error| serde::de::Error::custom(error.to_string()))?,
                ));
            } else {
                logs.push(CoreFilterChange::Log(
                    serde_json::from_value(item)
                        .map_err(|error| serde::de::Error::custom(error.to_string()))?,
                ));
            }
        }
        Ok(Self::Logs(logs))
    }
}

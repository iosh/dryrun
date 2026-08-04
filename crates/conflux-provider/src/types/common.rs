use std::{fmt, str::FromStr};

use alloy_primitives::B256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SelectorError {
    #[error("epoch number is missing the 0x prefix")]
    MissingHexPrefix,
    #[error("invalid epoch number {0:?}")]
    InvalidEpoch(String),
    #[error("invalid block hash selector {0:?}")]
    InvalidBlockHash(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EpochNumber {
    Number(u64),
    Earliest,
    LatestCheckpoint,
    LatestFinalized,
    LatestConfirmed,
    LatestState,
    LatestMined,
}

impl EpochNumber {
    pub fn parse(value: &str) -> Result<Self, SelectorError> {
        match value {
            "earliest" => Ok(Self::Earliest),
            "latest_checkpoint" => Ok(Self::LatestCheckpoint),
            "latest_finalized" => Ok(Self::LatestFinalized),
            "latest_confirmed" => Ok(Self::LatestConfirmed),
            "latest_state" => Ok(Self::LatestState),
            "latest_mined" => Ok(Self::LatestMined),
            value if value.starts_with("0x") => u64::from_str_radix(&value[2..], 16)
                .map(Self::Number)
                .map_err(|_| SelectorError::InvalidEpoch(value.to_owned())),
            _ => Err(SelectorError::MissingHexPrefix),
        }
    }
}

impl From<u64> for EpochNumber {
    fn from(value: u64) -> Self {
        Self::Number(value)
    }
}

impl FromStr for EpochNumber {
    type Err = SelectorError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for EpochNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(number) => write!(formatter, "0x{number:x}"),
            Self::Earliest => formatter.write_str("earliest"),
            Self::LatestCheckpoint => formatter.write_str("latest_checkpoint"),
            Self::LatestFinalized => formatter.write_str("latest_finalized"),
            Self::LatestConfirmed => formatter.write_str("latest_confirmed"),
            Self::LatestState => formatter.write_str("latest_state"),
            Self::LatestMined => formatter.write_str("latest_mined"),
        }
    }
}

impl Serialize for EpochNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for EpochNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockHashOrEpochNumber {
    Epoch(EpochNumber),
    BlockHash {
        hash: B256,
        require_pivot: Option<bool>,
    },
}

impl From<EpochNumber> for BlockHashOrEpochNumber {
    fn from(value: EpochNumber) -> Self {
        Self::Epoch(value)
    }
}

impl Serialize for BlockHashOrEpochNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Epoch(epoch) => epoch.serialize(serializer),
            Self::BlockHash {
                hash,
                require_pivot: None,
            } => serializer.serialize_str(&format!("hash:{hash:#x}")),
            Self::BlockHash {
                hash,
                require_pivot: Some(require_pivot),
            } => HashSelector {
                block_hash: *hash,
                require_pivot: *require_pivot,
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for BlockHashOrEpochNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if let Some(value) = value.as_str() {
            if let Some(hash) = value.strip_prefix("hash:0x") {
                let hash = hash
                    .parse::<B256>()
                    .map_err(|error| serde::de::Error::custom(error.to_string()))?;
                return Ok(Self::BlockHash {
                    hash,
                    require_pivot: None,
                });
            }
            return EpochNumber::parse(value)
                .map(Self::Epoch)
                .map_err(serde::de::Error::custom);
        }

        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("expected epoch or block hash selector"))?;
        if let Some(epoch) = object.get("epochNumber") {
            let epoch = serde_json::from_value::<EpochNumber>(epoch.clone())
                .map_err(|error| serde::de::Error::custom(error.to_string()))?;
            return Ok(Self::Epoch(epoch));
        }
        let hash = object
            .get("blockHash")
            .ok_or_else(|| serde::de::Error::custom("missing blockHash"))?;
        let hash = serde_json::from_value::<B256>(hash.clone())
            .map_err(|error| serde::de::Error::custom(error.to_string()))?;
        let require_pivot = object
            .get("requirePivot")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| serde::de::Error::custom("requirePivot must be boolean"))
            })
            .transpose()?;
        Ok(Self::BlockHash {
            hash,
            require_pivot: Some(require_pivot.unwrap_or(true)),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct HashSelector {
    block_hash: B256,
    require_pivot: bool,
}

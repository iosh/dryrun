use std::{fmt, str::FromStr};

use alloy_primitives::{B256, Bytes, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::CoreAddress;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
}

impl Serialize for CoreTransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = match self {
            Self::Legacy => U256::ZERO,
            Self::AccessList => U256::from(1_u8),
            Self::DynamicFee => U256::from(2_u8),
        };
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CoreTransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = U256::deserialize(deserializer)?;
        match value {
            value if value.is_zero() => Ok(Self::Legacy),
            value if value == U256::from(1_u8) => Ok(Self::AccessList),
            value if value == U256::from(2_u8) => Ok(Self::DynamicFee),
            value => Err(serde::de::Error::custom(format!(
                "unsupported Core transaction type {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreAccessListItem {
    pub address: CoreAddress,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateGasAndCollateralRequest {
    pub from: CoreAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<CoreAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<U256>,
    pub value: U256,
    pub data: Bytes,
    pub nonce: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_limit: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<CoreAccessListItem>>,
    #[serde(rename = "type")]
    pub transaction_type: CoreTransactionType,
    pub chain_id: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_height: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreCallRequest {
    pub to: CoreAddress,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceCheckRequest {
    pub account: CoreAddress,
    pub contract: CoreAddress,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub storage_limit: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreBlock {
    pub hash: B256,
    pub height: U256,
    pub miner: CoreAddress,
    pub block_number: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub timestamp: U256,
    pub pos_reference: Option<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosBlock {
    pub hash: B256,
    pub height: U256,
    pub pivot_decision: Option<PosPivotDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PosPivotDecision {
    pub height: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAndCollateralEstimate {
    pub gas_limit: U256,
    pub gas_used: U256,
    pub storage_collateralized: U256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceCheck {
    pub will_pay_tx_fee: bool,
    pub will_pay_collateral: bool,
    pub is_balance_enough: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSupplyInfo {
    pub total_circulating: U256,
    pub total_issued: U256,
    pub total_staking: U256,
    pub total_collateral: U256,
    pub total_espace_tokens: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreCollateralInfo {
    pub total_storage_tokens: U256,
    pub converted_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorePoSEconomics {
    pub total_pos_staking_tokens: U256,
    pub distributable_pos_interest: U256,
    pub last_distribute_block: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreVoteParams {
    pub pow_base_reward: U256,
    pub interest_rate: U256,
    pub storage_point_prop: U256,
    pub base_fee_share_prop: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreAccount {
    pub address: CoreAddress,
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: B256,
    pub staking_balance: U256,
    pub collateral_for_storage: U256,
    pub accumulated_interest_return: U256,
    pub admin: CoreAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreSponsorInfo {
    pub sponsor_for_gas: CoreAddress,
    pub sponsor_for_collateral: CoreAddress,
    pub sponsor_gas_bound: U256,
    pub sponsor_balance_for_gas: U256,
    pub sponsor_balance_for_collateral: U256,
    pub available_storage_points: U256,
    pub used_storage_points: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositInfo {
    pub amount: U256,
    pub deposit_time: U256,
    pub accumulated_interest_rate: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteStakeInfo {
    pub amount: U256,
    pub unlock_block_number: U256,
}

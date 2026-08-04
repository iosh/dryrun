use alloy_primitives::{B256, Bytes, U256};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::CoreAddress;

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
pub struct BalanceCheckRequest {
    pub account: CoreAddress,
    pub contract: CoreAddress,
    pub gas_limit: U256,
    pub gas_price: U256,
    pub storage_limit: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreBlockTransactions {
    Hashes(Vec<B256>),
    Full(Vec<CoreRpcTransaction>),
}

impl Serialize for CoreBlockTransactions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Hashes(value) => value.serialize(serializer),
            Self::Full(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CoreBlockTransactions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let array = value
            .as_array()
            .ok_or_else(|| serde::de::Error::custom("expected block transactions array"))?;
        if array.first().is_some_and(serde_json::Value::is_string) {
            serde_json::from_value(value)
                .map(Self::Hashes)
                .map_err(|error| serde::de::Error::custom(error.to_string()))
        } else {
            serde_json::from_value(value)
                .map(Self::Full)
                .map_err(|error| serde::de::Error::custom(error.to_string()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRpcBlock {
    pub hash: B256,
    pub parent_hash: B256,
    pub height: U256,
    pub miner: CoreAddress,
    pub deferred_state_root: B256,
    pub deferred_receipts_root: B256,
    pub deferred_logs_bloom_hash: B256,
    pub blame: U256,
    pub transactions_root: B256,
    pub epoch_number: Option<U256>,
    pub block_number: Option<U256>,
    pub gas_limit: U256,
    pub gas_used: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub timestamp: U256,
    pub difficulty: U256,
    pub pow_quality: Option<U256>,
    pub referee_hashes: Vec<B256>,
    pub adaptive: bool,
    pub nonce: U256,
    pub transactions: CoreBlockTransactions,
    pub size: Option<U256>,
    pub custom: Vec<Bytes>,
    pub pos_reference: Option<B256>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoreSpace {
    Native,
    #[serde(rename = "evm")]
    Ethereum,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRpcTransaction {
    #[serde(rename = "type")]
    pub transaction_type: Option<U256>,
    pub space: Option<CoreSpace>,
    pub hash: B256,
    pub nonce: U256,
    pub block_hash: Option<B256>,
    pub transaction_index: Option<U256>,
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub value: U256,
    pub gas_price: U256,
    pub gas: U256,
    pub contract_created: Option<CoreAddress>,
    pub data: Bytes,
    pub storage_limit: U256,
    pub epoch_height: U256,
    pub chain_id: Option<U256>,
    pub status: Option<U256>,
    pub access_list: Option<Vec<CoreAccessListItem>>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub v: U256,
    pub r: U256,
    pub s: U256,
    pub y_parity: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreTransactionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<CoreAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<CoreAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_limit: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<CoreAccessListItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub transaction_type: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch_height: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreLog {
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
pub struct CoreStorageChange {
    pub address: CoreAddress,
    pub collaterals: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreReceipt {
    #[serde(rename = "type")]
    pub transaction_type: Option<U256>,
    pub transaction_hash: B256,
    pub index: U256,
    pub block_hash: B256,
    pub epoch_number: Option<U256>,
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub gas_used: U256,
    pub accumulated_gas_used: Option<U256>,
    pub gas_fee: U256,
    pub effective_gas_price: U256,
    pub contract_created: Option<CoreAddress>,
    pub logs: Vec<CoreLog>,
    pub logs_bloom: Bytes,
    pub state_root: B256,
    pub outcome_status: U256,
    pub tx_exec_error_msg: Option<String>,
    pub gas_covered_by_sponsor: bool,
    pub storage_covered_by_sponsor: bool,
    pub storage_collateralized: U256,
    pub storage_released: Vec<CoreStorageChange>,
    pub space: Option<CoreSpace>,
    pub burnt_gas_fee: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStatus {
    pub best_hash: B256,
    pub chain_id: U256,
    pub ethereum_space_chain_id: U256,
    pub network_id: U256,
    pub epoch_number: U256,
    pub block_number: U256,
    pub pending_tx_number: U256,
    pub latest_checkpoint: U256,
    pub latest_confirmed: U256,
    pub latest_state: U256,
    pub latest_finalized: U256,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreFeeHistory {
    pub oldest_epoch: U256,
    pub base_fee_per_gas: Vec<U256>,
    pub gas_used_ratio: Vec<f64>,
    pub reward: Vec<Vec<U256>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRewardInfo {
    pub block_hash: B256,
    pub author: CoreAddress,
    pub total_reward: U256,
    pub base_reward: U256,
    pub tx_fee: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorePendingInfo {
    pub local_nonce: U256,
    pub pending_count: U256,
    pub pending_nonce: U256,
    pub next_pending_tx: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorePendingTransactions {
    pub pending_transactions: Vec<CoreRpcTransaction>,
    pub first_tx_status: Option<String>,
    pub pending_count: U256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreStorageRoot {
    pub delta: CoreMptValue,
    pub intermediate: CoreMptValue,
    pub snapshot: Option<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreMptValue {
    None,
    Tombstone,
    Some(B256),
}

impl Serialize for CoreMptValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::None => serializer.serialize_none(),
            Self::Tombstone => serializer.serialize_str("TOMBSTONE"),
            Self::Some(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CoreMptValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::None);
        }
        if value.as_str() == Some("TOMBSTONE") {
            return Ok(Self::Tombstone);
        }
        serde_json::from_value(value)
            .map(Self::Some)
            .map_err(|error| serde::de::Error::custom(error.to_string()))
    }
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

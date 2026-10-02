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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasAndCollateralEstimate {
    pub gas_limit: U256,
    pub gas_used: U256,
    pub storage_collateralized: U256,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRpcBlock {
    pub hash: B256,
    pub epoch_number: Option<U256>,
    pub block_number: Option<U256>,
    pub miner: CoreAddress,
    pub timestamp: U256,
    pub base_fee_per_gas: Option<U256>,
    pub pos_reference: Option<B256>,
}

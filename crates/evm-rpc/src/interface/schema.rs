use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_serde::quantity;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod u256_hex {
    use alloy_primitives::U256;
    use serde::{Serialize, Serializer};

    pub(super) fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvmSimulateTransactionRequest {
    pub transaction: Transaction,
    pub block: Option<BlockRef>,
    pub options: Option<SimulateTransactionOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum BlockRef {
    Tag(String),
    Hash(BlockHashRef),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockHashRef {
    pub block_hash: B256,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulateTransactionOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_overrides: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_overrides: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transaction {
    #[serde(
        rename = "type",
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub tx_type: Option<u8>,
    #[serde(with = "quantity")]
    pub chain_id: u64,
    pub from: Address,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub nonce: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub gas: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub gas_price: Option<u128>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub max_fee_per_gas: Option<u128>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub max_priority_fee_per_gas: Option<u128>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub max_fee_per_blob_gas: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_versioned_hashes: Option<Vec<B256>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization_list: Option<Vec<SignedAuthorization>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedAuthorization {
    pub chain_id: U256,
    pub address: Address,
    #[serde(with = "quantity")]
    pub nonce: u64,
    #[serde(rename = "yParity", alias = "v", with = "quantity")]
    pub y_parity: u8,
    pub r: U256,
    pub s: U256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateTransactionResponse {
    pub execution: Execution,
    pub changes: Changes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde(with = "quantity")]
    pub chain_id: u64,
    pub block: EvmBlockContext,
    pub status: ExecutionStatus,
    #[serde(with = "quantity")]
    pub gas_used: u64,
    #[serde(with = "quantity")]
    pub gas_limit: u64,
    pub fee: U256,
    pub burnt_fee: U256,
    pub output: Bytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvmBlockContext {
    #[serde(with = "quantity")]
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExecutionStatus {
    Success,
    Failed,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFailure {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Changes {
    Complete { items: Vec<StateChange> },
    Unavailable { error: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StateChange {
    NativeBalance {
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        before: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        after: U256,
        currency: NativeCurrency,
    },
    AccountDelegation {
        account: Address,
        before: DelegationState,
        after: DelegationState,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DelegationState {
    pub delegate: Option<Address>,
    #[serde(with = "alloy_serde::quantity")]
    pub nonce: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

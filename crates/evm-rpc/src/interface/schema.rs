use alloy::{
    primitives::{Address, B256, Bytes, U256},
    serde::quantity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod u256_quantity {
    use alloy::primitives::U256;
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub chain_id: Option<u64>,
    pub from: Address,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "quantity::opt"
    )]
    pub nonce: Option<u64>,
    #[serde(with = "quantity")]
    pub gas: u64,
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
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateTransactionResponse {
    pub execution: Execution,
    #[serde(default)]
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde(with = "quantity")]
    pub chain_id: u64,
    pub block: SimulatedBlock,
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
pub struct SimulatedBlock {
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
#[serde(
    tag = "changeType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Change {
    Transfer {
        #[serde(flatten)]
        asset: TransferAsset,
        from: Address,
        to: Address,
    },
    Mint {
        #[serde(flatten)]
        asset: TokenMovementAsset,
        to: Address,
    },
    Burn {
        #[serde(flatten)]
        asset: TokenMovementAsset,
        from: Address,
    },
    Allowance {
        #[serde(flatten)]
        asset: AllowanceAsset,
        owner: Address,
        spender: Address,
    },
    TokenApproval {
        #[serde(flatten)]
        asset: TokenApprovalAsset,
    },
    OperatorApproval {
        #[serde(flatten)]
        asset: OperatorApprovalAsset,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum TransferAsset {
    Native {
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: NativeMetadata,
    },
    Erc20 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum TokenMovementAsset {
    Erc20 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum AllowanceAsset {
    Erc20 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount_before: U256,
        #[serde(serialize_with = "u256_quantity::serialize")]
        raw_amount_after: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum TokenApprovalAsset {
    Erc721 {
        contract_address: Address,
        #[serde(serialize_with = "u256_quantity::serialize")]
        token_id: U256,
        approved_address_before: Option<Address>,
        approved_address_after: Option<Address>,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum OperatorApprovalAsset {
    Erc721 {
        contract_address: Address,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc20Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc721CollectionMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

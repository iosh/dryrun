use alloy::{
    primitives::{Address, B256, Bytes, U256},
    serde::quantity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateTransactionResponse {
    pub execution: Execution,
    #[serde(default)]
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde(with = "quantity")]
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: SimulationStatus,
    #[serde(with = "quantity")]
    pub gas_used: u64,
    #[serde(with = "quantity")]
    pub gas_limit: u64,
    pub output: Bytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ExecutionError>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimulatedBlock {
    #[serde(with = "quantity")]
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SimulationStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Change {
    Transfer {
        asset: Asset,
        from: Address,
        to: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Mint {
        asset: Asset,
        to: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Burn {
        asset: Asset,
        from: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Approval {
        asset: Asset,
        owner: Address,
        spender: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    ApprovalForAll {
        collection: Collection,
        owner: Address,
        operator: Address,
        approved: bool,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct NativeAssetDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc20AssetDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc721CollectionDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc1155CollectionDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NftTokenDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Asset {
    Native {
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<NativeAssetDisplay>,
    },
    Erc20 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<Erc20AssetDisplay>,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc721CollectionDisplay>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<NftTokenDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc1155CollectionDisplay>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<NftTokenDisplay>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Collection {
    Erc721 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc721CollectionDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc1155CollectionDisplay>,
    },
}

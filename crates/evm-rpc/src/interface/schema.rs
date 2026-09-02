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
    pub state: EvmState,
    pub transaction: CompletedTransaction,
    pub outcome: Outcome,
    pub changes: Changes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvmState {
    #[serde(with = "quantity")]
    pub block_number: u64,
    pub block_hash: B256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CompletedTransaction {
    Legacy(LegacyTransaction),
    Eip2930(Eip2930Transaction),
    Eip1559(Eip1559Transaction),
    Eip4844(Eip4844Transaction),
    Eip7702(Eip7702Transaction),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompletedTransactionBase {
    #[serde(rename = "type", with = "quantity")]
    pub tx_type: u8,
    #[serde(with = "quantity")]
    pub chain_id: u64,
    pub from: Address,
    pub to: Option<Address>,
    #[serde(with = "quantity")]
    pub nonce: u64,
    #[serde(with = "quantity")]
    pub gas: u64,
    pub value: U256,
    pub data: Bytes,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacyTransaction {
    #[serde(flatten)]
    pub base: CompletedTransactionBase,
    #[serde(with = "quantity")]
    pub gas_price: u128,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Eip2930Transaction {
    #[serde(flatten)]
    pub base: CompletedTransactionBase,
    #[serde(with = "quantity")]
    pub gas_price: u128,
    pub access_list: Vec<AccessListItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Eip1559Transaction {
    #[serde(flatten)]
    pub base: CompletedTransactionBase,
    #[serde(with = "quantity")]
    pub max_fee_per_gas: u128,
    #[serde(with = "quantity")]
    pub max_priority_fee_per_gas: u128,
    pub access_list: Vec<AccessListItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Eip4844Transaction {
    #[serde(flatten)]
    pub base: CompletedTransactionBase,
    #[serde(with = "quantity")]
    pub max_fee_per_gas: u128,
    #[serde(with = "quantity")]
    pub max_priority_fee_per_gas: u128,
    #[serde(with = "quantity")]
    pub max_fee_per_blob_gas: u128,
    pub access_list: Vec<AccessListItem>,
    pub blob_versioned_hashes: Vec<B256>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Eip7702Transaction {
    #[serde(flatten)]
    pub base: CompletedTransactionBase,
    #[serde(with = "quantity")]
    pub max_fee_per_gas: u128,
    #[serde(with = "quantity")]
    pub max_priority_fee_per_gas: u128,
    pub access_list: Vec<AccessListItem>,
    pub authorization_list: Vec<SignedAuthorization>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum Outcome {
    Success(SuccessOutcome),
    Reverted(RevertedOutcome),
    Failed(FailedOutcome),
    Rejected { error: String },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SuccessOutcome {
    Call(SuccessCallOutcome),
    Create(SuccessCreateOutcome),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuccessCallOutcome {
    #[serde(flatten)]
    pub accounting: ExecutionAccounting,
    pub return_data: Bytes,
    pub logs: Vec<SimulationLog>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuccessCreateOutcome {
    #[serde(flatten)]
    pub accounting: ExecutionAccounting,
    pub contract_address: Address,
    pub runtime_code: Bytes,
    pub logs: Vec<SimulationLog>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RevertedOutcome {
    #[serde(flatten)]
    pub accounting: ExecutionAccounting,
    pub revert_data: Bytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailedOutcome {
    #[serde(flatten)]
    pub accounting: ExecutionAccounting,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionAccounting {
    #[serde(with = "quantity")]
    pub gas_used: u64,
    #[serde(with = "quantity")]
    pub effective_gas_price: u128,
    pub gas_fee: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burnt_gas_fee: Option<U256>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub blob: Option<BlobGasAccounting>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlobGasAccounting {
    #[serde(with = "quantity")]
    pub blob_gas_used: u64,
    #[serde(with = "quantity")]
    pub blob_gas_price: u128,
    pub blob_gas_fee: U256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimulationLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
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
    NativeTransfer {
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    AccountDelegation {
        account: Address,
        before: DelegationState,
        after: DelegationState,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Approval {
        contract_address: Address,
        owner: Address,
        spender: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        before: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        after: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: Address,
        owner: Address,
        before: Option<Address>,
        after: Option<Address>,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        before: bool,
        after: bool,
    },
    Erc1155TransferSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155MintSingle {
        contract_address: Address,
        operator: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155BurnSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
    Erc1155MintBatch {
        contract_address: Address,
        operator: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
    Erc1155BurnBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Erc1155TransferItem {
    #[serde(serialize_with = "u256_hex::serialize")]
    pub token_id: U256,
    #[serde(serialize_with = "u256_hex::serialize")]
    pub raw_amount: U256,
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

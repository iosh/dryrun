use alloy_primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use evm_simulation::TransactionRequest as Transaction;

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

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct EvmSimulateTransactionResponse(
    pub(crate)  std::sync::Arc<
        simulation_core::simulation::Simulation<
            evm_simulation::EvmBlockContext,
            evm_simulation::TypedTransaction,
            evm_simulation::TransactionRequest,
            evm_simulation::EvmExecutionOutcome,
            evm_simulation::EvmTransactionRejection,
            Vec<StateChange>,
            evm_simulation::EvmChangeDerivationError,
        >,
    >,
);

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

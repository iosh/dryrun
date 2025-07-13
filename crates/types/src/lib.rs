use alloy::{
    primitives::{Address, Bytes, Log, U64, U256},
    rpc::types::{BlockId, BlockOverrides, TransactionRequest, state::StateOverride},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct EvmSimulateInput {
    pub transaction: TransactionRequest,
    pub block_id: Option<BlockId>,
    pub state_overrides: Option<StateOverride>,
    pub block_overrides: Option<BlockOverrides>,
}

impl EvmSimulateInput {
    pub fn new(
        transaction: TransactionRequest,
        block_id: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<BlockOverrides>,
    ) -> Self {
        Self {
            transaction,
            block_id,
            state_overrides,
            block_overrides,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TraceActionType {
    Call,
    StaticCall,
    DelegateCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateOutput {
    pub status: bool,

    pub gas_used: U64,
    pub block_number: U256,

    pub logs: Vec<Log>,
    pub trace: Vec<CallTraceItem>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub state_changes: Vec<StateChange>,
    // pub asset_changes: Vec<serde_json::Value>,
    // pub balance_changes: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallTraceItem {
    #[serde(rename(serialize = "type"))]
    pub action_type: TraceActionType,
    pub from: Address,
    pub to: Address,
    pub gas: U64,
    pub gas_used: U64,
    pub value: U256,
    pub input: Bytes,

    pub output: Bytes,
    pub subtraces: usize,
    pub trace_address: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateChange {
    pub address: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<ValueChange<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<ValueChange<U256>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub storage: Vec<StorageChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageChange {
    pub slot: U256,
    pub previous_value: U256,
    pub new_value: U256,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueChange<T> {
    pub previous_value: T,
    pub new_value: T,
}

use alloy::{
    primitives::{Address, B256, Bytes, Log, U256},
    rpc::types::{
        BlockId, BlockOverrides, TransactionRequest, state::StateOverride,
        trace::parity::ActionType,
    },
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateOutput {
    pub status: bool,

    pub gas_used: u64,
    pub block_number: U256,

    pub logs: Vec<Log>,
    // pub trace: Vec<CallTraceItem>,

    // pub state_changes: Vec<StateChange>,
    // pub asset_changes: Vec<serde_json::Value>,
    // pub balance_changes: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallTraceItem {
    #[serde(rename(serialize = "type"))]
    pub action_type: ActionType,
    pub from: Address,
    pub to: Address,
    pub gas: u64,
    pub gas_used: u64,
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
    pub slot: B256,
    pub previous_value: B256,
    pub new_value: B256,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueChange<T> {
    pub previous_value: T,
    pub new_value: T,
}

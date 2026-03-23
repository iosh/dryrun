use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvmSimulateTransactionRequest {
    #[serde(default)]
    pub block: Option<BlockRef>,
    pub transaction: Transaction,
    #[serde(default)]
    pub options: Option<SimulationOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockRef {
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SimulationOptions {
    #[serde(default)]
    pub include_logs: Option<bool>,
    #[serde(default)]
    pub include_asset_changes: Option<bool>,
    #[serde(default)]
    pub include_trace: Option<bool>,
    #[serde(default)]
    pub include_state_changes: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub tx_type: String,
    pub chain_id: String,
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
    pub nonce: String,
    pub gas: String,
    pub value: String,
    pub data: String,
    #[serde(default)]
    pub access_list: Option<Vec<AccessListItem>>,
    #[serde(default)]
    pub gas_price: Option<String>,
    #[serde(default)]
    pub max_fee_per_gas: Option<String>,
    #[serde(default)]
    pub max_priority_fee_per_gas: Option<String>,
    #[serde(default)]
    pub blob_versioned_hashes: Option<Vec<String>>,
    #[serde(default)]
    pub max_fee_per_blob_gas: Option<String>,
    #[serde(default)]
    pub sidecar: Option<serde_json::Value>,
    #[serde(default)]
    pub authorization_list: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessListItem {
    pub address: String,
    pub storage_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmSimulateTransactionResponse {
    pub chain_id: String,
    pub block: SimulatedBlock,
    pub status: SimulationStatus,
    pub gas_used: String,
    pub gas_limit: String,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<SimulationFailure>,
    #[serde(default)]
    pub logs: Vec<RawLog>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulatedBlock {
    pub number: String,
    pub hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SimulationStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationFailure {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLog {
    pub log_index: String,
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

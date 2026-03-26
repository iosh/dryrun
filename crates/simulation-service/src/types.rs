use alloy_primitives::{Address, B256, Bytes, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRef {
    Latest,
    Number(u64),
    Hash(B256),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationOptions {
    pub include_logs: bool,
    pub include_asset_changes: bool,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self {
            include_logs: true,
            include_asset_changes: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmTransaction {
    pub tx_type: EvmTransactionType,
    pub chain_id: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionInput {
    pub block: BlockRef,
    pub transaction: EvmTransaction,
    pub options: SimulationOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLog {
    pub log_index: u64,
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Native,
    Erc20,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetChangeType {
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetChangeAsset {
    pub token_address: Address,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetChange {
    pub asset_type: AssetType,
    pub change_type: AssetChangeType,
    pub from: Address,
    pub to: Address,
    pub amount: U256,
    pub asset: Option<AssetChangeAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEvmTransactionOutput {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: SimulationStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Bytes,
    pub failure: Option<SimulationFailure>,
    pub logs: Vec<RawLog>,
    pub asset_changes: Vec<AssetChange>,
}

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
pub struct EvmExecutionInput {
    pub block: BlockRef,
    pub transaction: EvmTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionLog {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceType {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    Create,
    Create2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceItem {
    pub trace_type: TraceType,
    pub from: Address,
    pub to: Option<Address>,
    pub code_address: Option<Address>,
    pub value: U256,
    pub input: Bytes,
    pub output: Bytes,
    pub gas: u64,
    pub gas_used: u64,
    pub trace_address: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionOutput {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: EvmExecutionStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Bytes,
    pub failure: Option<EvmExecutionFailure>,
    pub logs: Vec<EvmExecutionLog>,
    pub asset_changes: Vec<AssetChange>,
    pub trace: Vec<TraceItem>,
}

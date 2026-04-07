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
    pub requested_chain_id: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
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

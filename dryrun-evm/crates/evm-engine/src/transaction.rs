use alloy_primitives::{Address, B256, Bytes, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRef {
    Latest,
    Number(u64),
    Hash(B256),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmTransaction {
    pub chain_id: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub data: Bytes,
    pub variant: EvmTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmTransactionVariant {
    Legacy {
        gas_price: u128,
    },
    Eip2930 {
        gas_price: u128,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionInput {
    pub block: BlockRef,
    pub transaction: EvmTransaction,
}

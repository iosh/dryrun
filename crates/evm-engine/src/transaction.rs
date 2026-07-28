use alloy_primitives::{Address, B256, Bytes, U256};
use simulation_transaction::TransactionVariant;

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

pub type EvmTransactionVariant = TransactionVariant<u128, AccessListItem>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionInput {
    pub block: crate::ResolvedBlock,
    pub transaction: EvmTransaction,
}

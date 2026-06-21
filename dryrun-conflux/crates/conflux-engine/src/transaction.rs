use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
    Eip7702,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceTransaction {
    pub tx_type: EspaceTransactionType,
    pub requested_chain_id: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<U256>,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: EspaceTransaction,
}

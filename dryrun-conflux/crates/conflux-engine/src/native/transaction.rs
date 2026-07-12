use cfx_bytes::Bytes;
use cfx_types::{Address, U256};

use crate::espace::AccessListItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEpochRef {
    LatestState,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTransactionVariant {
    Cip155 {
        gas_price: U256,
    },
    Cip2930 {
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    Cip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub storage_limit: u64,
    pub epoch_height: u64,
    pub chain_id: u32,
    pub variant: NativeTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateNativeTransactionInput {
    pub epoch: NativeEpochRef,
    pub transaction: NativeTransaction,
}

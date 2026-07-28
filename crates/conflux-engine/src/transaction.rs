use cfx_bytes::Bytes;
use cfx_types::{Address, U256};
pub use primitives::AccessListItem;
use simulation_transaction::TransactionVariant;

pub type ConfluxTransactionVariant = TransactionVariant<U256, AccessListItem>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxTransactionBody {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u32,
    pub variant: ConfluxTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxTransaction {
    pub body: ConfluxTransactionBody,
    pub gas_limit: U256,
}

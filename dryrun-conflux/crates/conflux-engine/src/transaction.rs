use cfx_bytes::Bytes;
use cfx_types::{Address, U256};

use crate::{
    espace::{AccessListItem, EspaceTransaction, EspaceTransactionVariant},
    execution::{EspaceTransactionInput, NativeTransactionInput},
};
use primitives::{
    AccessListItem as PrimitiveAccessListItem,
    transaction::{
        Action, Cip1559Transaction, Cip2930Transaction, Eip155Transaction, Eip1559Transaction,
        Eip2930Transaction, EthereumTransaction, NativeTransaction as PrimitiveNativeTransaction,
        TypedNativeTransaction,
    },
};

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

pub fn build_espace_transaction_input(input: EspaceTransaction) -> EspaceTransactionInput {
    let sender = input.from;
    let tx = build_ethereum_transaction(input);

    EspaceTransactionInput { tx, sender }
}

pub fn build_native_transaction_input(input: NativeTransaction) -> NativeTransactionInput {
    let sender = input.from;
    let tx = build_typed_native_transaction(input);

    NativeTransactionInput { tx, sender }
}

fn build_typed_native_transaction(input: NativeTransaction) -> TypedNativeTransaction {
    let NativeTransaction {
        to,
        nonce,
        gas_limit,
        value,
        data,
        storage_limit,
        epoch_height,
        chain_id,
        variant,
        ..
    } = input;

    let action = action_from_to(to);

    match variant {
        NativeTransactionVariant::Cip155 { gas_price } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price,
                gas: gas_limit,
                action,
                value,
                storage_limit,
                epoch_height,
                chain_id,
                data,
            })
        }
        NativeTransactionVariant::Cip2930 {
            gas_price,
            access_list,
        } => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price,
            gas: gas_limit,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: map_access_list(access_list),
        }),
        NativeTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas: gas_limit,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: map_access_list(access_list),
        }),
    }
}

fn build_ethereum_transaction(input: EspaceTransaction) -> EthereumTransaction {
    let EspaceTransaction {
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id,
        variant,
        ..
    } = input;

    let action = action_from_to(to);

    match variant {
        EspaceTransactionVariant::Legacy { gas_price } => {
            EthereumTransaction::Eip155(Eip155Transaction {
                nonce,
                gas_price,
                gas: gas_limit,
                action,
                value,
                chain_id: Some(chain_id),
                data,
            })
        }
        EspaceTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => EthereumTransaction::Eip2930(Eip2930Transaction {
            chain_id,
            nonce,
            gas_price,
            gas: gas_limit,
            action,
            value,
            data,
            access_list: map_access_list(access_list),
        }),
        EspaceTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => EthereumTransaction::Eip1559(Eip1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas: gas_limit,
            action,
            value,
            data,
            access_list: map_access_list(access_list),
        }),
    }
}
fn action_from_to(to: Option<Address>) -> Action {
    to.map_or(Action::Create, Action::Call)
}

fn map_access_list(items: Vec<AccessListItem>) -> Vec<PrimitiveAccessListItem> {
    items
        .into_iter()
        .map(|item| PrimitiveAccessListItem {
            address: item.address,
            storage_keys: item.storage_keys,
        })
        .collect()
}

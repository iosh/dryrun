use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use primitives::{
    AccessListItem as PrimitiveAccessListItem,
    transaction::{
        Action, Cip1559Transaction, Cip2930Transaction,
        NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
    },
};

use crate::execution::CoreSpaceTransactionInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceEpochRef {
    LatestState,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceTransactionVariant {
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
pub struct CoreSpaceTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub storage_limit: u64,
    pub epoch_height: u64,
    pub chain_id: u32,
    pub variant: CoreSpaceTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateCoreSpaceTransactionInput {
    pub epoch: CoreSpaceEpochRef,
    pub transaction: CoreSpaceTransaction,
}

pub(crate) fn build_core_space_transaction_input(
    input: CoreSpaceTransaction,
) -> CoreSpaceTransactionInput {
    let sender = input.from;
    let tx = build_typed_core_space_transaction(input);

    CoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(input: CoreSpaceTransaction) -> TypedNativeTransaction {
    let CoreSpaceTransaction {
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
        CoreSpaceTransactionVariant::Cip155 { gas_price } => {
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
        CoreSpaceTransactionVariant::Cip2930 {
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
        CoreSpaceTransactionVariant::Cip1559 {
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

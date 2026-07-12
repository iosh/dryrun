use cfx_bytes::Bytes;
use cfx_types::{Address, U256};

use crate::{
    espace::{AccessListItem, EspaceTransaction, EspaceTransactionType},
    error::ConfluxEngineError,
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

pub fn build_espace_transaction_input(
    input: EspaceTransaction,
    fallback_chain_id: u32,
) -> Result<EspaceTransactionInput, ConfluxEngineError> {
    let sender = input.from;
    let tx = build_ethereum_transaction(input, fallback_chain_id)?;

    Ok(EspaceTransactionInput { tx, sender })
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

fn build_ethereum_transaction(
    input: EspaceTransaction,
    fallback_chain_id: u32,
) -> Result<EthereumTransaction, ConfluxEngineError> {
    let chain_id = resolve_chain_id(input.requested_chain_id, fallback_chain_id)?;
    let nonce = input.nonce.unwrap_or_default();

    Ok(match input.tx_type {
        EspaceTransactionType::Legacy => EthereumTransaction::Eip155(Eip155Transaction {
            nonce,
            gas_price: input.gas_price.unwrap_or_else(U256::one),
            gas: input.gas_limit,
            action: action_from_to(input.to),
            value: input.value,
            chain_id: Some(chain_id),
            data: input.data,
        }),
        EspaceTransactionType::AccessList => EthereumTransaction::Eip2930(Eip2930Transaction {
            chain_id,
            nonce,
            gas_price: input.gas_price.unwrap_or_else(U256::one),
            gas: input.gas_limit,
            action: action_from_to(input.to),
            value: input.value,
            data: input.data,
            access_list: map_access_list(input.access_list),
        }),
        EspaceTransactionType::DynamicFee => EthereumTransaction::Eip1559(Eip1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: input.max_priority_fee_per_gas.unwrap_or_default(),
            max_fee_per_gas: input
                .max_fee_per_gas
                .or(input.max_priority_fee_per_gas)
                .or(input.gas_price)
                .unwrap_or_else(U256::one),
            gas: input.gas_limit,
            action: action_from_to(input.to),
            value: input.value,
            data: input.data,
            access_list: map_access_list(input.access_list),
        }),
        EspaceTransactionType::Eip7702 => {
            return Err(ConfluxEngineError::UnsupportedTransactionType {
                tx_type: "EIP-7702",
            });
        }
    })
}

fn action_from_to(to: Option<Address>) -> Action {
    to.map_or(Action::Create, Action::Call)
}

fn resolve_chain_id(
    requested_chain_id: Option<u64>,
    fallback_chain_id: u32,
) -> Result<u32, ConfluxEngineError> {
    requested_chain_id.map_or(Ok(fallback_chain_id), |chain_id| {
        u32::try_from(chain_id).map_err(|_| ConfluxEngineError::InvalidTransaction {
            message: format!("requested chain_id exceeds u32: {chain_id}"),
        })
    })
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

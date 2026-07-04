use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

use crate::{
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
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeEpochRef {
    LatestState,
    Number(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
    Eip7702,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTransactionType {
    Legacy,
    Cip2930,
    Cip1559,
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
pub struct NativeTransaction {
    pub tx_type: NativeTransactionType,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<U256>,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub storage_limit: Option<u64>,
    pub access_list: Vec<AccessListItem>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub requested_chain_id: Option<u64>,
    pub requested_epoch_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: EspaceTransaction,
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

pub fn build_native_transaction_input(
    input: NativeTransaction,
    fallback_chain_id: u32,
    fallback_epoch_height: u64,
) -> Result<NativeTransactionInput, ConfluxEngineError> {
    let sender = input.from;
    let tx = build_typed_native_transaction(input, fallback_chain_id, fallback_epoch_height)?;

    Ok(NativeTransactionInput { tx, sender })
}

fn build_typed_native_transaction(
    input: NativeTransaction,
    fallback_chain_id: u32,
    fallback_epoch_height: u64,
) -> Result<TypedNativeTransaction, ConfluxEngineError> {
    let chain_id = resolve_chain_id(input.requested_chain_id, fallback_chain_id)?;
    let epoch_height = input
        .requested_epoch_height
        .unwrap_or(fallback_epoch_height);
    let nonce = input.nonce.unwrap_or_default();
    let storage_limit = input.storage_limit.unwrap_or(u64::MAX);
    let action = action_from_to(input.to);

    Ok(match input.tx_type {
        NativeTransactionType::Legacy => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price: input.gas_price.unwrap_or_else(U256::one),
                gas: input.gas_limit,
                action,
                value: input.value,
                storage_limit,
                epoch_height,
                chain_id,
                data: input.data,
            })
        }
        NativeTransactionType::Cip2930 => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price: input.gas_price.unwrap_or_else(U256::one),
            gas: input.gas_limit,
            action,
            value: input.value,
            storage_limit,
            epoch_height,
            chain_id,
            data: input.data,
            access_list: map_access_list(input.access_list),
        }),
        NativeTransactionType::Cip1559 => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas: input.max_priority_fee_per_gas.unwrap_or_default(),
            max_fee_per_gas: input
                .max_fee_per_gas
                .or(input.max_priority_fee_per_gas)
                .or(input.gas_price)
                .unwrap_or_else(U256::one),
            gas: input.gas_limit,
            action,
            value: input.value,
            storage_limit,
            epoch_height,
            chain_id,
            data: input.data,
            access_list: map_access_list(input.access_list),
        }),
    })
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

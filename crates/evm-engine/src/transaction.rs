use alloy_primitives::{Address, B256, Bytes, U256};
use simulation_transaction::{
    SimulationTransaction, TransactionField, TransactionKind as SimulationTransactionKind,
};
use thiserror::Error;

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
    pub block: crate::ResolvedBlock,
    pub transaction: EvmTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("transaction field `{field}` must fit into an unsigned {max_bits}-bit integer")]
pub struct TransactionInputError {
    field: TransactionField,
    max_bits: u16,
}

impl TransactionInputError {
    fn out_of_range(field: TransactionField, max_bits: u16) -> Self {
        Self { field, max_bits }
    }
}

impl TryFrom<SimulationTransaction> for EvmTransaction {
    type Error = TransactionInputError;

    fn try_from(transaction: SimulationTransaction) -> Result<Self, Self::Error> {
        let SimulationTransaction {
            from,
            to,
            nonce,
            gas_limit,
            value,
            input,
            chain_id,
            kind,
        } = transaction;

        let chain_id = to_u64(chain_id, TransactionField::ChainId)?;
        let nonce = to_u64(nonce, TransactionField::Nonce)?;
        let gas_limit = to_u64(gas_limit, TransactionField::GasLimit)?;

        let variant = match kind {
            SimulationTransactionKind::Legacy { gas_price } => EvmTransactionVariant::Legacy {
                gas_price: to_u128(gas_price, TransactionField::GasPrice)?,
            },
            SimulationTransactionKind::AccessList {
                gas_price,
                access_list,
            } => EvmTransactionVariant::Eip2930 {
                gas_price: to_u128(gas_price, TransactionField::GasPrice)?,
                access_list: to_evm_access_list(access_list),
            },
            SimulationTransactionKind::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => EvmTransactionVariant::Eip1559 {
                max_fee_per_gas: to_u128(max_fee_per_gas, TransactionField::MaxFeePerGas)?,
                max_priority_fee_per_gas: to_u128(
                    max_priority_fee_per_gas,
                    TransactionField::MaxPriorityFeePerGas,
                )?,
                access_list: to_evm_access_list(access_list),
            },
        };

        Ok(Self {
            chain_id,
            from,
            to,
            nonce,
            gas_limit,
            value,
            data: input,
            variant,
        })
    }
}

fn to_u64(value: U256, field: TransactionField) -> Result<u64, TransactionInputError> {
    u64::try_from(value).map_err(|_| TransactionInputError::out_of_range(field, 64))
}

fn to_u128(value: U256, field: TransactionField) -> Result<u128, TransactionInputError> {
    u128::try_from(value).map_err(|_| TransactionInputError::out_of_range(field, 128))
}

fn to_evm_access_list(items: Vec<simulation_transaction::AccessListItem>) -> Vec<AccessListItem> {
    items
        .into_iter()
        .map(|item| AccessListItem {
            address: item.address,
            storage_keys: item.storage_keys,
        })
        .collect()
}

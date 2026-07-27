use super::{EspaceExecutionFailure, EspaceExecutionFailureCode};
use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use primitives::{
    AccessListItem as PrimitiveAccessListItem,
    transaction::{
        Action, Eip155Transaction, Eip1559Transaction, Eip2930Transaction, EthereumTransaction,
    },
};
use simulation_transaction::{SimulationTransaction, TransactionKind as SimulationTransactionKind};

use crate::{
    execution::EspaceTransactionInput,
    transaction_adapter::{
        TransactionInputError, TransactionInputField, to_cfx_address, to_cfx_bytes, to_cfx_h256,
        to_cfx_u256,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceTransactionVariant {
    Legacy {
        gas_price: U256,
    },
    Eip2930 {
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u32,
    pub variant: EspaceTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: EspaceTransaction,
}

impl TryFrom<SimulationTransaction> for EspaceTransaction {
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

        let chain_id = u32::try_from(chain_id)
            .map_err(|_| TransactionInputError::out_of_range(TransactionInputField::ChainId, 32))?;

        let variant = match kind {
            SimulationTransactionKind::Legacy { gas_price } => EspaceTransactionVariant::Legacy {
                gas_price: to_cfx_u256(gas_price),
            },
            SimulationTransactionKind::AccessList {
                gas_price,
                access_list,
            } => EspaceTransactionVariant::Eip2930 {
                gas_price: to_cfx_u256(gas_price),
                access_list: to_espace_access_list(access_list),
            },
            SimulationTransactionKind::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => EspaceTransactionVariant::Eip1559 {
                max_fee_per_gas: to_cfx_u256(max_fee_per_gas),
                max_priority_fee_per_gas: to_cfx_u256(max_priority_fee_per_gas),
                access_list: to_espace_access_list(access_list),
            },
        };

        Ok(Self {
            from: to_cfx_address(from),
            to: to.map(to_cfx_address),
            nonce: to_cfx_u256(nonce),
            gas_limit: to_cfx_u256(gas_limit),
            value: to_cfx_u256(value),
            data: to_cfx_bytes(input),
            chain_id,
            variant,
        })
    }
}

fn to_espace_access_list(
    items: Vec<simulation_transaction::AccessListItem>,
) -> Vec<AccessListItem> {
    items
        .into_iter()
        .map(|item| AccessListItem {
            address: to_cfx_address(item.address),
            storage_keys: item.storage_keys.into_iter().map(to_cfx_h256).collect(),
        })
        .collect()
}

pub(crate) fn build_espace_transaction_input(input: EspaceTransaction) -> EspaceTransactionInput {
    let sender = input.from;
    let tx = build_ethereum_transaction(input);

    EspaceTransactionInput { tx, sender }
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

pub(crate) fn validate_espace_transaction(
    transaction: &EspaceTransaction,
    expected_chain_id: u32,
) -> Result<(), EspaceExecutionFailure> {
    if transaction.chain_id != expected_chain_id {
        return Err(EspaceExecutionFailure {
            code: EspaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match engine chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    match &transaction.variant {
        EspaceTransactionVariant::Legacy { gas_price }
        | EspaceTransactionVariant::Eip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        EspaceTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if max_fee_per_gas.is_zero() {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction max fee per gas must be greater than zero".to_string(),
                    reason: None,
                });
            }

            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::PriorityFeeExceedsMaxFee,
                    message: format!(
                        "max priority fee per gas {} exceeds max fee per gas {}",
                        max_priority_fee_per_gas, max_fee_per_gas
                    ),
                    reason: None,
                });
            }
        }
    }

    Ok(())
}

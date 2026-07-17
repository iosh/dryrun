use super::{EspaceExecutionFailure, EspaceExecutionFailureCode};
use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use primitives::{
    AccessListItem as PrimitiveAccessListItem,
    transaction::{
        Action, Eip155Transaction, Eip1559Transaction, Eip2930Transaction, EthereumTransaction,
    },
};

use crate::execution::EspaceTransactionInput;

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

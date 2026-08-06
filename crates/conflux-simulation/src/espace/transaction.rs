use super::{EspaceExecutionFailure, EspaceExecutionFailureCode};
use primitives::transaction::{
    Action, Eip155Transaction, Eip1559Transaction, Eip2930Transaction, EthereumTransaction,
};

use crate::{
    execution::EspaceTransactionInput,
    primitive::{access_list_to_cfx, address_to_cfx, u256_to_cfx},
};
pub use simulation_transaction::{
    Transaction as EspaceTransaction, TransactionVariant as EspaceTransactionVariant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

pub(crate) fn build_espace_transaction_input(
    input: EspaceTransaction,
    chain_id: u32,
) -> EspaceTransactionInput {
    let sender = address_to_cfx(input.from);
    let tx = build_ethereum_transaction(input, chain_id);

    EspaceTransactionInput { tx, sender }
}

fn build_ethereum_transaction(input: EspaceTransaction, chain_id: u32) -> EthereumTransaction {
    let EspaceTransaction {
        to,
        nonce,
        gas_limit,
        value,
        data,
        variant,
        ..
    } = input;

    let action = to.map_or(Action::Create, |address| {
        Action::Call(address_to_cfx(address))
    });
    let nonce = nonce.into();
    let gas = gas_limit.into();
    let value = u256_to_cfx(value);
    let data = data.to_vec();

    match variant {
        EspaceTransactionVariant::Legacy { gas_price } => {
            EthereumTransaction::Eip155(Eip155Transaction {
                nonce,
                gas_price: gas_price.into(),
                gas,
                action,
                value,
                chain_id: Some(chain_id),
                data,
            })
        }
        EspaceTransactionVariant::AccessList {
            gas_price,
            access_list,
        } => EthereumTransaction::Eip2930(Eip2930Transaction {
            chain_id,
            nonce,
            gas_price: gas_price.into(),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
        EspaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => EthereumTransaction::Eip1559(Eip1559Transaction {
            chain_id,
            nonce,
            max_priority_fee_per_gas: max_priority_fee_per_gas.into(),
            max_fee_per_gas: max_fee_per_gas.into(),
            gas,
            action,
            value,
            data,
            access_list: access_list_to_cfx(access_list),
        }),
    }
}

pub(crate) fn validate_espace_transaction(
    transaction: &EspaceTransaction,
    expected_chain_id: u32,
) -> Result<(), EspaceExecutionFailure> {
    if transaction.chain_id != u64::from(expected_chain_id) {
        return Err(EspaceExecutionFailure {
            code: EspaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match simulation chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    match &transaction.variant {
        EspaceTransactionVariant::Legacy { gas_price }
        | EspaceTransactionVariant::AccessList { gas_price, .. } => {
            if *gas_price == 0 {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        EspaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if *max_fee_per_gas == 0 {
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

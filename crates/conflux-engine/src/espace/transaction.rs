use super::{EspaceExecutionFailure, EspaceExecutionFailureCode};
use cfx_types::Address;
use primitives::transaction::{
    Action, Eip155Transaction, Eip1559Transaction, Eip2930Transaction, EthereumTransaction,
};

use crate::{ConfluxTransaction, ConfluxTransactionVariant, execution::EspaceTransactionInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

pub type EspaceTransaction = ConfluxTransaction;
pub type EspaceTransactionVariant = ConfluxTransactionVariant;

pub(crate) fn build_espace_transaction_input(input: EspaceTransaction) -> EspaceTransactionInput {
    let sender = input.body.from;
    let tx = build_ethereum_transaction(input);

    EspaceTransactionInput { tx, sender }
}

fn build_ethereum_transaction(input: EspaceTransaction) -> EthereumTransaction {
    let EspaceTransaction { body, gas_limit } = input;
    let crate::ConfluxTransactionBody {
        to,
        nonce,
        value,
        data,
        chain_id,
        variant,
        ..
    } = body;

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
        EspaceTransactionVariant::AccessList {
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
            access_list,
        }),
        EspaceTransactionVariant::DynamicFee {
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
            access_list,
        }),
    }
}

fn action_from_to(to: Option<Address>) -> Action {
    to.map_or(Action::Create, Action::Call)
}

pub(crate) fn validate_espace_transaction(
    transaction: &EspaceTransaction,
    expected_chain_id: u32,
) -> Result<(), EspaceExecutionFailure> {
    let transaction = &transaction.body;

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
        | EspaceTransactionVariant::AccessList { gas_price, .. } => {
            if gas_price.is_zero() {
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

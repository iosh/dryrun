use alloy_primitives::U256;

use super::{
    DynamicFees, EspaceContext, EspaceTransactionCommon, EspaceTransactionCompletionError,
    EspaceTransactionInput, EspaceTransactionRequest, EspaceTypedTransaction, FeeInput,
    PartialTransactionCommon, TxType,
};
use crate::state::{ConfluxSimulationProvider, EspaceEstimateTransaction};

pub(crate) async fn complete_transaction(
    input: EspaceTransactionInput,
    provider: &ConfluxSimulationProvider,
    context: &EspaceContext,
    chain_id: u64,
) -> Result<EspaceTypedTransaction, EspaceTransactionCompletionError> {
    match input {
        EspaceTransactionInput::Complete(transaction) => {
            transaction.check_type_requirements()?;
            if transaction.transaction_type() == TxType::Eip4844 {
                return Err(
                    EspaceTransactionCompletionError::UnsupportedTransactionType {
                        transaction_type: TxType::Eip4844,
                    },
                );
            }
            Ok(transaction)
        }
        EspaceTransactionInput::Partial(transaction) => {
            complete_partial_transaction(transaction, provider, context, chain_id).await
        }
    }
}

async fn complete_partial_transaction(
    transaction: EspaceTransactionRequest,
    provider: &ConfluxSimulationProvider,
    context: &EspaceContext,
    chain_id: u64,
) -> Result<EspaceTypedTransaction, EspaceTransactionCompletionError> {
    let default_type = if !context.base_fee_per_gas().is_zero() {
        TxType::Eip1559
    } else {
        TxType::Legacy
    };
    let transaction_type = transaction.transaction_type(default_type)?;
    if transaction_type == TxType::Eip4844 {
        return Err(
            EspaceTransactionCompletionError::UnsupportedTransactionType { transaction_type },
        );
    }
    let EspaceTransactionRequest {
        common,
        fees,
        access_list,
        authorization_list,
        ..
    } = transaction;
    let PartialTransactionCommon {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id: requested_chain_id,
    } = common;
    let FeeInput {
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
    } = fees;
    let chain_id = requested_chain_id.unwrap_or(chain_id);
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => {
            let value = provider
                .eth_get_transaction_count(from, context.state_block())
                .await
                .map_err(|source| EspaceTransactionCompletionError::NonceLookup {
                    block_number: context.public_context.number,
                    source,
                })?;
            u64::try_from(value).map_err(|_| EspaceTransactionCompletionError::NonceOutOfRange {
                block_number: context.public_context.number,
                value: crate::primitive::u256_from_cfx(value),
            })?
        }
    };
    let value = value.unwrap_or_default();
    let input = input.unwrap_or_default();
    let needs_gas_estimate = gas_limit.is_none();
    let common = EspaceTransactionCommon {
        from,
        to,
        nonce,
        gas_limit: gas_limit.unwrap_or_default(),
        value,
        input,
        chain_id,
    };
    let access_list = access_list.unwrap_or_default();
    let authorization_list = authorization_list.unwrap_or_default();
    let mut transaction = match transaction_type {
        TxType::Legacy => EspaceTypedTransaction::Legacy {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
        },
        TxType::Eip2930 => EspaceTypedTransaction::Eip2930 {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
            access_list,
        },
        TxType::Eip1559 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, context, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            EspaceTypedTransaction::Eip1559 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
                access_list,
            }
        }
        TxType::Eip7702 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, context, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            EspaceTypedTransaction::Eip7702 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
                access_list,
                authorization_list,
            }
        }
        TxType::Eip4844 => {
            return Err(
                EspaceTransactionCompletionError::UnsupportedTransactionType { transaction_type },
            );
        }
    };

    if needs_gas_estimate {
        let estimate = provider
            .eth_estimate_gas(
                EspaceEstimateTransaction {
                    transaction: &transaction,
                },
                context.state_block(),
            )
            .await
            .map_err(|source| EspaceTransactionCompletionError::GasEstimation {
                block_number: context.public_context.number,
                source,
            })?;
        transaction.common_mut().gas_limit = u64::try_from(estimate).map_err(|_| {
            EspaceTransactionCompletionError::GasEstimateOutOfRange {
                block_number: context.public_context.number,
                value: crate::primitive::u256_from_cfx(estimate),
            }
        })?;
    }

    Ok(transaction)
}

async fn complete_gas_price(
    provider: &ConfluxSimulationProvider,
    gas_price: Option<U256>,
) -> Result<U256, EspaceTransactionCompletionError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider
            .eth_gas_price()
            .await
            .map(crate::primitive::u256_from_cfx)
            .map_err(|source| EspaceTransactionCompletionError::GasPriceSuggestion { source }),
    }
}

async fn complete_dynamic_fees(
    provider: &ConfluxSimulationProvider,
    context: &EspaceContext,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
) -> Result<(U256, U256), EspaceTransactionCompletionError> {
    let max_priority_fee_per_gas = match max_priority_fee_per_gas {
        Some(value) => value,
        None => {
            let value = provider
                .eth_max_priority_fee_per_gas()
                .await
                .map_err(
                    |source| EspaceTransactionCompletionError::PriorityFeeSuggestion { source },
                )?;
            crate::primitive::u256_from_cfx(value)
        }
    };
    let max_fee_per_gas = match max_fee_per_gas {
        Some(value) => value,
        None => suggested_max_fee_per_gas(context, max_priority_fee_per_gas)?,
    };

    Ok((max_fee_per_gas, max_priority_fee_per_gas))
}

fn suggested_max_fee_per_gas(
    context: &EspaceContext,
    max_priority_fee_per_gas: U256,
) -> Result<U256, EspaceTransactionCompletionError> {
    let base_fee = context.base_fee_per_gas();
    crate::primitive::u256_from_cfx(base_fee)
        .checked_mul(U256::from(2))
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or(EspaceTransactionCompletionError::MaxFeePerGasOverflow)
}

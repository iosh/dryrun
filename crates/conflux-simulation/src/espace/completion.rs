use alloy_primitives::U256;

use super::{
    EspaceCompleteTransaction, EspacePartialTransaction, EspaceTransactionCommon,
    EspaceTransactionCompletionError, EspaceTransactionInput, ResolvedEspaceContext, TxType,
};
use crate::state::{ConfluxSimulationProvider, EspaceEstimateTransaction};

pub(crate) async fn complete_transaction(
    input: EspaceTransactionInput,
    provider: &ConfluxSimulationProvider,
    context: &ResolvedEspaceContext,
    chain_id: u64,
) -> Result<EspaceCompleteTransaction, EspaceTransactionCompletionError> {
    match input {
        EspaceTransactionInput::Complete(transaction) => {
            transaction.validate()?;
            Ok(transaction)
        }
        EspaceTransactionInput::Partial(transaction) => {
            complete_partial_transaction(transaction, provider, context, chain_id).await
        }
    }
}

async fn complete_partial_transaction(
    transaction: EspacePartialTransaction,
    provider: &ConfluxSimulationProvider,
    context: &ResolvedEspaceContext,
    chain_id: u64,
) -> Result<EspaceCompleteTransaction, EspaceTransactionCompletionError> {
    let transaction_type = transaction.transaction_type.unwrap_or_else(|| {
        transaction.preferred_type(
            context
                .base_fee_per_gas()
                .map_or(false, |fee| !fee.is_zero()),
        )
    });
    if transaction_type == TxType::Eip4844 {
        return Err(
            EspaceTransactionCompletionError::UnsupportedTransactionType { transaction_type },
        );
    }
    transaction.validate(transaction_type)?;

    let EspacePartialTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id: requested_chain_id,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        max_fee_per_blob_gas: _,
        access_list,
        blob_versioned_hashes: _,
        authorization_list,
        transaction_type: _,
    } = transaction;
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
        TxType::Legacy => EspaceCompleteTransaction::Legacy {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
        },
        TxType::Eip2930 => EspaceCompleteTransaction::Eip2930 {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
            access_list,
        },
        TxType::Eip1559 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, context, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            EspaceCompleteTransaction::Eip1559 {
                common,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            }
        }
        TxType::Eip7702 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, context, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            EspaceCompleteTransaction::Eip7702 {
                common,
                max_fee_per_gas,
                max_priority_fee_per_gas,
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
    context: &ResolvedEspaceContext,
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
    context: &ResolvedEspaceContext,
    max_priority_fee_per_gas: U256,
) -> Result<U256, EspaceTransactionCompletionError> {
    let base_fee =
        context
            .base_fee_per_gas()
            .ok_or(EspaceTransactionCompletionError::MissingBaseFee {
                block_number: context.public_context.number,
            })?;
    crate::primitive::u256_from_cfx(base_fee)
        .checked_mul(U256::from(2))
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or(EspaceTransactionCompletionError::MaxFeePerGasOverflow)
}

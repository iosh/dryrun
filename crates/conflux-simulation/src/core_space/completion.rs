use alloy_primitives::U256;

use super::{
    CoreSpaceCompleteTransaction, CoreSpacePartialTransaction, CoreSpacePartialTransactionCommon,
    CoreSpaceTransactionCommon, CoreSpaceTransactionCompletionError, CoreSpaceTransactionInput,
    ResolvedCoreSpaceContext,
};
use crate::{
    primitive::u256_from_cfx,
    state::{ConfluxSimulationProvider, CoreSpaceEstimateTransaction, CoreSpaceResourceEstimate},
};

pub(crate) async fn complete_transaction(
    input: CoreSpaceTransactionInput,
    provider: &ConfluxSimulationProvider,
    context: &ResolvedCoreSpaceContext,
    chain_id: u32,
) -> Result<CoreSpaceCompleteTransaction, CoreSpaceTransactionCompletionError> {
    match input {
        CoreSpaceTransactionInput::Complete(transaction) => Ok(transaction),
        CoreSpaceTransactionInput::Partial(transaction) => {
            complete_partial_transaction(transaction, provider, context, chain_id).await
        }
    }
}

async fn complete_partial_transaction(
    transaction: CoreSpacePartialTransaction,
    provider: &ConfluxSimulationProvider,
    context: &ResolvedCoreSpaceContext,
    chain_id: u32,
) -> Result<CoreSpaceCompleteTransaction, CoreSpaceTransactionCompletionError> {
    let (transaction_kind, common) = match transaction {
        CoreSpacePartialTransaction::Cip155 { common, gas_price } => {
            let gas_price = complete_gas_price(provider, gas_price).await?;
            (PartialTransactionKind::Cip155 { gas_price }, common)
        }
        CoreSpacePartialTransaction::Cip2930 {
            common,
            gas_price,
            access_list,
        } => {
            let gas_price = complete_gas_price(provider, gas_price).await?;
            (
                PartialTransactionKind::Cip2930 {
                    gas_price,
                    access_list,
                },
                common,
            )
        }
        CoreSpacePartialTransaction::Cip1559 {
            common,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let max_priority_fee_per_gas = match max_priority_fee_per_gas {
                Some(value) => value,
                None => u256_from_cfx(provider.cfx_max_priority_fee_per_gas().await?),
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => suggested_max_fee_per_gas(context, max_priority_fee_per_gas)?,
            };
            (
                PartialTransactionKind::Cip1559 {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    access_list,
                },
                common,
            )
        }
    };
    let CoreSpacePartialTransactionCommon {
        from,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id: requested_chain_id,
        storage_limit,
        epoch_height,
    } = common;
    let chain_id = requested_chain_id.unwrap_or(chain_id);
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => u256_from_cfx(
            provider
                .cfx_get_next_nonce(from, context.state_pivot())
                .await?,
        ),
    };
    let value = value.unwrap_or_default();
    let data = data.unwrap_or_default();
    let epoch_height = epoch_height.unwrap_or_else(|| context.epoch_height());

    let mut completed = match transaction_kind {
        PartialTransactionKind::Cip155 { gas_price } => CoreSpaceCompleteTransaction::Cip155 {
            common: CoreSpaceTransactionCommon {
                from,
                to,
                nonce,
                gas_limit: gas_limit.unwrap_or_default(),
                value,
                data,
                chain_id,
                storage_limit: storage_limit.unwrap_or_default(),
                epoch_height,
            },
            gas_price,
        },
        PartialTransactionKind::Cip2930 {
            gas_price,
            access_list,
        } => CoreSpaceCompleteTransaction::Cip2930 {
            common: CoreSpaceTransactionCommon {
                from,
                to,
                nonce,
                gas_limit: gas_limit.unwrap_or_default(),
                value,
                data,
                chain_id,
                storage_limit: storage_limit.unwrap_or_default(),
                epoch_height,
            },
            gas_price,
            access_list,
        },
        PartialTransactionKind::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => CoreSpaceCompleteTransaction::Cip1559 {
            common: CoreSpaceTransactionCommon {
                from,
                to,
                nonce,
                gas_limit: gas_limit.unwrap_or_default(),
                value,
                data,
                chain_id,
                storage_limit: storage_limit.unwrap_or_default(),
                epoch_height,
            },
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        },
    };

    if gas_limit.is_none() || storage_limit.is_none() {
        let estimate = provider
            .cfx_estimate_gas_and_collateral(
                CoreSpaceEstimateTransaction {
                    transaction: &completed,
                    gas_limit,
                    storage_limit,
                },
                context.state_epoch(),
            )
            .await?;
        let (gas_limit, storage_limit) =
            complete_estimated_resources(gas_limit, storage_limit, estimate)?;
        completed.common_mut().gas_limit = gas_limit;
        completed.common_mut().storage_limit = storage_limit;
    }

    Ok(completed)
}

enum PartialTransactionKind {
    Cip155 {
        gas_price: U256,
    },
    Cip2930 {
        gas_price: U256,
        access_list: Vec<super::CoreSpaceAccessListItem>,
    },
    Cip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<super::CoreSpaceAccessListItem>,
    },
}

fn complete_estimated_resources(
    gas_limit: Option<U256>,
    storage_limit: Option<u64>,
    estimate: CoreSpaceResourceEstimate,
) -> Result<(U256, u64), CoreSpaceTransactionCompletionError> {
    let gas_limit = gas_limit.unwrap_or(estimate.gas_limit);
    let storage_limit = match storage_limit {
        Some(storage_limit) => storage_limit,
        None => u64::try_from(estimate.storage_limit).map_err(|_| {
            CoreSpaceTransactionCompletionError::StorageLimitOutOfRange {
                value: estimate.storage_limit,
            }
        })?,
    };
    Ok((gas_limit, storage_limit))
}

async fn complete_gas_price(
    provider: &ConfluxSimulationProvider,
    gas_price: Option<U256>,
) -> Result<U256, CoreSpaceTransactionCompletionError> {
    match gas_price {
        Some(value) => Ok(value),
        None => Ok(u256_from_cfx(provider.cfx_gas_price().await?)),
    }
}

fn suggested_max_fee_per_gas(
    context: &ResolvedCoreSpaceContext,
    max_priority_fee_per_gas: U256,
) -> Result<U256, CoreSpaceTransactionCompletionError> {
    let base_fee =
        context
            .base_fee_per_gas()
            .ok_or(CoreSpaceTransactionCompletionError::MissingBaseFee {
                epoch_number: context.public_context.epoch_number,
            })?;
    u256_from_cfx(base_fee)
        .checked_mul(U256::from(2))
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or(CoreSpaceTransactionCompletionError::MaxFeePerGasOverflow)
}

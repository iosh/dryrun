use alloy_primitives::{Address, Bytes, U256};
use cfx_types::U256 as CfxU256;
use simulation_transaction::{
    Transaction, TransactionRequest, TransactionVariant, TransactionVariantRequest,
};

use super::{CoreSpaceSimulationContext, EspaceSimulationContext};
use crate::{
    ConfluxSimulationError, ConfluxSimulationProvider,
    core_space::{
        CoreSpaceTransaction, CoreSpaceTransactionRequest, CoreSpaceTransactionVariant,
        CoreSpaceTransactionVariantRequest, validate_core_space_transaction_network,
    },
};

#[derive(Debug)]
struct TransactionWithoutGasLimit {
    from: Address,
    to: Option<Address>,
    nonce: u64,
    value: U256,
    data: Bytes,
    chain_id: u64,
    variant: TransactionVariant,
}

impl TransactionWithoutGasLimit {
    fn into_transaction(self, gas_limit: u64) -> Transaction {
        Transaction {
            from: self.from,
            to: self.to,
            nonce: self.nonce,
            gas_limit,
            value: self.value,
            data: self.data,
            chain_id: self.chain_id,
            variant: self.variant,
        }
    }
}

pub(crate) async fn complete_espace_transaction(
    provider: &ConfluxSimulationProvider,
    context: &EspaceSimulationContext,
    request: TransactionRequest,
) -> Result<Transaction, ConfluxSimulationError> {
    let (transaction, gas_limit) =
        complete_espace_without_gas_limit(provider, context, request).await?;
    let gas_limit = match gas_limit {
        Some(gas_limit) => gas_limit,
        None => {
            let estimate = provider
                .eth_estimate_gas(
                    transaction.from,
                    transaction.to,
                    transaction.nonce,
                    transaction.value,
                    &transaction.data,
                    transaction.chain_id,
                    &transaction.variant,
                    context.state_block(),
                )
                .await?;
            u64::try_from(estimate).map_err(|_| {
                unsupported_value("eSpace gas estimate", estimate, CfxU256::from(u64::MAX))
            })?
        }
    };

    Ok(transaction.into_transaction(gas_limit))
}

pub(crate) async fn complete_core_space_transaction(
    provider: &ConfluxSimulationProvider,
    context: &CoreSpaceSimulationContext,
    request: CoreSpaceTransactionRequest,
    requested_storage_limit: Option<u64>,
    requested_epoch_height: Option<u64>,
) -> Result<CoreSpaceTransaction, ConfluxSimulationError> {
    validate_core_space_transaction_network(&request, provider.provider_network())?;
    let CoreSpaceTransactionRequest {
        from,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id,
        variant,
    } = request;
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => {
            let nonce = provider
                .cfx_get_next_nonce(from, context.state_epoch())
                .await?;
            u64::try_from(nonce).map_err(|_| {
                unsupported_value(
                    "Core Space transaction nonce",
                    nonce,
                    CfxU256::from(u64::MAX),
                )
            })?
        }
    };
    let variant = complete_core_space_transaction_variant(provider, context, variant).await?;
    let value = value.unwrap_or_default();
    let data = data.unwrap_or_default();
    let epoch_height = requested_epoch_height.unwrap_or_else(|| context.epoch_height());

    let (gas_limit, storage_limit) = match (gas_limit, requested_storage_limit) {
        (Some(gas_limit), Some(storage_limit)) => (gas_limit, storage_limit),
        (gas_limit, storage_limit) => {
            let estimate = provider
                .cfx_estimate_gas_and_collateral(
                    from,
                    to,
                    nonce,
                    value,
                    &data,
                    chain_id,
                    &variant,
                    epoch_height,
                    gas_limit,
                    storage_limit,
                    context.state_epoch(),
                )
                .await?;

            (
                match gas_limit {
                    Some(gas_limit) => gas_limit,
                    None => u64::try_from(estimate.gas_limit).map_err(|_| {
                        unsupported_value(
                            "Core Space gas estimate",
                            estimate.gas_limit,
                            CfxU256::from(u64::MAX),
                        )
                    })?,
                },
                storage_limit.unwrap_or(estimate.storage_limit),
            )
        }
    };

    Ok(CoreSpaceTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id,
        variant,
        storage_limit,
        epoch_height,
    })
}

async fn complete_espace_without_gas_limit(
    provider: &ConfluxSimulationProvider,
    context: &EspaceSimulationContext,
    request: TransactionRequest,
) -> Result<(TransactionWithoutGasLimit, Option<u64>), ConfluxSimulationError> {
    let TransactionRequest {
        from,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id,
        variant,
    } = request;
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => {
            let nonce = provider
                .eth_get_transaction_count(from, context.state_block())
                .await?;
            u64::try_from(nonce).map_err(|_| {
                unsupported_value("eSpace transaction nonce", nonce, CfxU256::from(u64::MAX))
            })?
        }
    };
    let variant = complete_espace_transaction_variant(provider, context, variant).await?;

    Ok((
        TransactionWithoutGasLimit {
            from,
            to,
            nonce,
            value: value.unwrap_or_default(),
            data: data.unwrap_or_default(),
            chain_id,
            variant,
        },
        gas_limit,
    ))
}

async fn complete_espace_transaction_variant(
    provider: &ConfluxSimulationProvider,
    context: &EspaceSimulationContext,
    variant: TransactionVariantRequest,
) -> Result<TransactionVariant, ConfluxSimulationError> {
    match variant {
        TransactionVariantRequest::Legacy { gas_price } => Ok(TransactionVariant::Legacy {
            gas_price: suggested_espace_gas_price(provider, gas_price).await?,
        }),
        TransactionVariantRequest::AccessList {
            gas_price,
            access_list,
        } => Ok(TransactionVariant::AccessList {
            gas_price: suggested_espace_gas_price(provider, gas_price).await?,
            access_list,
        }),
        TransactionVariantRequest::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let max_priority_fee_per_gas = match max_priority_fee_per_gas {
                Some(value) => value,
                None => {
                    let fee = provider.eth_max_priority_fee_per_gas().await?;
                    u128::try_from(fee).map_err(|_| {
                        unsupported_value(
                            "eSpace max priority fee per gas",
                            fee,
                            CfxU256::from(u128::MAX),
                        )
                    })?
                }
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => {
                    suggested_dynamic_fee_cap(context.base_fee_per_gas(), max_priority_fee_per_gas)?
                }
            };

            Ok(TransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            })
        }
    }
}

async fn complete_core_space_transaction_variant(
    provider: &ConfluxSimulationProvider,
    context: &CoreSpaceSimulationContext,
    variant: CoreSpaceTransactionVariantRequest,
) -> Result<CoreSpaceTransactionVariant, ConfluxSimulationError> {
    match variant {
        CoreSpaceTransactionVariantRequest::Legacy { gas_price } => {
            Ok(CoreSpaceTransactionVariant::Legacy {
                gas_price: suggested_core_gas_price(provider, gas_price).await?,
            })
        }
        CoreSpaceTransactionVariantRequest::AccessList {
            gas_price,
            access_list,
        } => Ok(CoreSpaceTransactionVariant::AccessList {
            gas_price: suggested_core_gas_price(provider, gas_price).await?,
            access_list,
        }),
        CoreSpaceTransactionVariantRequest::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let max_priority_fee_per_gas = match max_priority_fee_per_gas {
                Some(value) => value,
                None => {
                    let fee = provider.cfx_max_priority_fee_per_gas().await?;
                    u128::try_from(fee).map_err(|_| {
                        unsupported_value(
                            "Core Space max priority fee per gas",
                            fee,
                            CfxU256::from(u128::MAX),
                        )
                    })?
                }
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => {
                    suggested_dynamic_fee_cap(context.base_fee_per_gas(), max_priority_fee_per_gas)?
                }
            };

            Ok(CoreSpaceTransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            })
        }
    }
}

async fn suggested_espace_gas_price(
    provider: &ConfluxSimulationProvider,
    gas_price: Option<u128>,
) -> Result<u128, ConfluxSimulationError> {
    match gas_price {
        Some(value) => Ok(value),
        None => {
            let gas_price = provider.eth_gas_price().await?;
            u128::try_from(gas_price).map_err(|_| {
                unsupported_value("eSpace gas price", gas_price, CfxU256::from(u128::MAX))
            })
        }
    }
}

async fn suggested_core_gas_price(
    provider: &ConfluxSimulationProvider,
    gas_price: Option<u128>,
) -> Result<u128, ConfluxSimulationError> {
    match gas_price {
        Some(value) => Ok(value),
        None => {
            let gas_price = provider.cfx_gas_price().await?;
            u128::try_from(gas_price).map_err(|_| {
                unsupported_value("Core Space gas price", gas_price, CfxU256::from(u128::MAX))
            })
        }
    }
}

fn suggested_dynamic_fee_cap(
    base_fee: Option<CfxU256>,
    max_priority_fee_per_gas: u128,
) -> Result<u128, ConfluxSimulationError> {
    let base_fee = base_fee.ok_or_else(|| {
        ConfluxSimulationError::transaction_completion_failed(
            "simulation context does not provide a base fee for dynamic fee completion",
        )
    })?;
    let base_fee = u128::try_from(base_fee)
        .map_err(|_| unsupported_value("base fee per gas", base_fee, CfxU256::from(u128::MAX)))?;
    base_fee
        .checked_mul(2)
        .and_then(|fee| fee.checked_add(max_priority_fee_per_gas))
        .ok_or_else(|| {
            ConfluxSimulationError::transaction_completion_failed(
                "calculated dynamic fee exceeds the simulator maximum \
                 340282366920938463463374607431768211455",
            )
        })
}

fn unsupported_value(field: &str, value: CfxU256, max: CfxU256) -> ConfluxSimulationError {
    ConfluxSimulationError::transaction_completion_failed(format!(
        "{field} value {value} exceeds the simulator maximum {max}"
    ))
}

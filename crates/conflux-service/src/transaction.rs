use cfx_bytes::Bytes;
use cfx_types::{Address, U256};
pub use conflux_engine::AccessListItem;
use conflux_engine::{
    ConfluxRpcError, ConfluxTransaction, ConfluxTransactionBody, ConfluxTransactionVariant,
    CoreSpaceSimulationContext, EspaceSimulationContext, HttpConfluxProvider,
    core_space::CoreSpaceTransaction,
};
use simulation_transaction::{TransactionVariant, TransactionVariantRequest};

use crate::{ConfluxServiceError, core_space::CoreSpaceTransactionRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluxTransactionRequest {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<U256>,
    pub gas_limit: Option<U256>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub chain_id: u32,
    pub variant: TransactionVariantRequest<U256, AccessListItem>,
}

pub(crate) async fn complete_espace_transaction(
    provider: &HttpConfluxProvider,
    context: &EspaceSimulationContext,
    request: ConfluxTransactionRequest,
) -> Result<ConfluxTransaction, ConfluxServiceError> {
    let (body, gas_limit) = complete_transaction_body(
        request,
        TransactionCompletionContext::Espace { provider, context },
    )
    .await?;
    let gas_limit = match gas_limit {
        Some(gas_limit) => gas_limit,
        None => {
            provider
                .eth_estimate_gas(&body, context.state_block())
                .await?
        }
    };

    Ok(ConfluxTransaction { body, gas_limit })
}

pub(crate) async fn complete_core_space_transaction(
    provider: &HttpConfluxProvider,
    context: &CoreSpaceSimulationContext,
    request: CoreSpaceTransactionRequest,
) -> Result<CoreSpaceTransaction, ConfluxServiceError> {
    let CoreSpaceTransactionRequest {
        transaction,
        storage_limit,
        epoch_height,
    } = request;
    let (body, gas_limit) = complete_transaction_body(
        transaction,
        TransactionCompletionContext::CoreSpace { provider, context },
    )
    .await?;
    let epoch_height = epoch_height.unwrap_or_else(|| context.epoch_height());

    let (gas_limit, storage_limit) = match (gas_limit, storage_limit) {
        (Some(gas_limit), Some(storage_limit)) => (gas_limit, storage_limit),
        (gas_limit, storage_limit) => {
            let estimate = provider
                .cfx_estimate_gas_and_collateral(
                    &body,
                    epoch_height,
                    gas_limit,
                    storage_limit,
                    context.state_epoch(),
                )
                .await?;

            (
                gas_limit.unwrap_or(estimate.gas_limit),
                storage_limit.unwrap_or(estimate.storage_limit),
            )
        }
    };

    Ok(CoreSpaceTransaction {
        transaction: ConfluxTransaction { body, gas_limit },
        storage_limit,
        epoch_height,
    })
}

async fn complete_transaction_body(
    request: ConfluxTransactionRequest,
    context: TransactionCompletionContext<'_>,
) -> Result<(ConfluxTransactionBody, Option<U256>), ConfluxServiceError> {
    let ConfluxTransactionRequest {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
        variant,
    } = request;
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => context.nonce(from).await?,
    };
    let variant = complete_transaction_variant(context, variant).await?;

    Ok((
        ConfluxTransactionBody {
            from,
            to,
            nonce,
            value: value.unwrap_or_default(),
            data: input.unwrap_or_default(),
            chain_id,
            variant,
        },
        gas_limit,
    ))
}

#[derive(Clone, Copy)]
enum TransactionCompletionContext<'a> {
    Espace {
        provider: &'a HttpConfluxProvider,
        context: &'a EspaceSimulationContext,
    },
    CoreSpace {
        provider: &'a HttpConfluxProvider,
        context: &'a CoreSpaceSimulationContext,
    },
}

impl TransactionCompletionContext<'_> {
    async fn nonce(self, address: Address) -> Result<U256, ConfluxRpcError> {
        match self {
            Self::Espace { provider, context } => {
                provider
                    .eth_get_transaction_count(address, context.state_block())
                    .await
            }
            Self::CoreSpace { provider, context } => {
                provider
                    .cfx_get_next_nonce(address, context.state_epoch())
                    .await
            }
        }
    }

    async fn gas_price(self) -> Result<U256, ConfluxRpcError> {
        match self {
            Self::Espace { provider, .. } => provider.eth_gas_price().await,
            Self::CoreSpace { provider, .. } => provider.cfx_gas_price().await,
        }
    }

    async fn max_priority_fee_per_gas(self) -> Result<U256, ConfluxRpcError> {
        match self {
            Self::Espace { provider, .. } => provider.eth_max_priority_fee_per_gas().await,
            Self::CoreSpace { provider, .. } => provider.cfx_max_priority_fee_per_gas().await,
        }
    }

    fn base_fee_per_gas(self) -> Option<U256> {
        match self {
            Self::Espace { context, .. } => context.base_fee_per_gas(),
            Self::CoreSpace { context, .. } => context.base_fee_per_gas(),
        }
    }
}

async fn complete_transaction_variant(
    context: TransactionCompletionContext<'_>,
    variant: TransactionVariantRequest<U256, AccessListItem>,
) -> Result<ConfluxTransactionVariant, ConfluxServiceError> {
    match variant {
        TransactionVariantRequest::Legacy { gas_price } => Ok(TransactionVariant::Legacy {
            gas_price: suggested_gas_price(context, gas_price).await?,
        }),
        TransactionVariantRequest::AccessList {
            gas_price,
            access_list,
        } => Ok(TransactionVariant::AccessList {
            gas_price: suggested_gas_price(context, gas_price).await?,
            access_list,
        }),
        TransactionVariantRequest::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let max_priority_fee_per_gas = match max_priority_fee_per_gas {
                Some(value) => value,
                None => context.max_priority_fee_per_gas().await?,
            };
            let max_fee_per_gas = match max_fee_per_gas {
                Some(value) => value,
                None => suggested_dynamic_fee_cap(context, max_priority_fee_per_gas)?,
            };

            Ok(TransactionVariant::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            })
        }
    }
}

async fn suggested_gas_price(
    context: TransactionCompletionContext<'_>,
    gas_price: Option<U256>,
) -> Result<U256, ConfluxRpcError> {
    match gas_price {
        Some(value) => Ok(value),
        None => context.gas_price().await,
    }
}

fn suggested_dynamic_fee_cap(
    context: TransactionCompletionContext<'_>,
    max_priority_fee_per_gas: U256,
) -> Result<U256, ConfluxServiceError> {
    let base_fee = context.base_fee_per_gas().ok_or_else(|| {
        ConfluxServiceError::transaction_completion(
            "simulation context does not provide a base fee for dynamic fee completion",
        )
    })?;
    let (base_fee, multiplication_overflow) = base_fee.overflowing_mul(2.into());
    let (max_fee_per_gas, addition_overflow) = base_fee.overflowing_add(max_priority_fee_per_gas);

    if multiplication_overflow || addition_overflow {
        return Err(ConfluxServiceError::transaction_completion(
            "calculated dynamic fee exceeds an unsigned 256-bit integer",
        ));
    }

    Ok(max_fee_per_gas)
}

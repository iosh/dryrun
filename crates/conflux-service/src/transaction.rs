use cfx_bytes::Bytes;
use cfx_types::{Address, U256};
pub use conflux_engine::AccessListItem;
use conflux_engine::{
    ConfluxEngine, ConfluxTransaction, ConfluxTransactionBody, ConfluxTransactionVariant,
    ResolvedCoreSpaceContext, ResolvedEspaceContext, core_space::CoreSpaceTransaction,
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

pub(crate) async fn resolve_espace_transaction(
    engine: &ConfluxEngine,
    context: &ResolvedEspaceContext,
    request: ConfluxTransactionRequest,
) -> Result<ConfluxTransaction, ConfluxServiceError> {
    let transaction = request
        .resolve(ConfluxSpaceContext::Espace { engine, context })
        .await?;
    let gas_limit = match transaction.gas_limit {
        Some(gas_limit) => gas_limit,
        None => engine.eth_estimate_gas(context, &transaction.body).await?,
    };

    Ok(ConfluxTransaction {
        body: transaction.body,
        gas_limit,
    })
}

pub(crate) async fn resolve_core_space_transaction(
    engine: &ConfluxEngine,
    context: &ResolvedCoreSpaceContext,
    request: CoreSpaceTransactionRequest,
) -> Result<CoreSpaceTransaction, ConfluxServiceError> {
    let CoreSpaceTransactionRequest {
        transaction,
        storage_limit,
        epoch_height,
    } = request;
    let transaction = transaction
        .resolve(ConfluxSpaceContext::CoreSpace { engine, context })
        .await?;
    let epoch_height = epoch_height.unwrap_or_else(|| context.epoch_height());

    let (gas_limit, storage_limit) = match (transaction.gas_limit, storage_limit) {
        (Some(gas_limit), Some(storage_limit)) => (gas_limit, storage_limit),
        (gas_limit, storage_limit) => {
            let estimate = engine
                .cfx_estimate_gas_and_collateral(
                    context,
                    &transaction.body,
                    epoch_height,
                    gas_limit,
                    storage_limit,
                )
                .await?;

            (
                gas_limit.unwrap_or(estimate.gas_limit),
                storage_limit.unwrap_or(estimate.storage_limit),
            )
        }
    };

    Ok(CoreSpaceTransaction {
        transaction: ConfluxTransaction {
            body: transaction.body,
            gas_limit,
        },
        storage_limit,
        epoch_height,
    })
}

struct ResolvedBaseTransaction {
    body: ConfluxTransactionBody,
    gas_limit: Option<U256>,
}

impl ConfluxTransactionRequest {
    async fn resolve(
        self,
        context: ConfluxSpaceContext<'_>,
    ) -> Result<ResolvedBaseTransaction, ConfluxServiceError> {
        let Self {
            from,
            to,
            nonce,
            gas_limit,
            value,
            input,
            chain_id,
            variant,
        } = self;
        let nonce = match nonce {
            Some(nonce) => nonce,
            None => context.nonce(from).await?,
        };
        let variant = resolve_fees(context, variant).await?;

        Ok(ResolvedBaseTransaction {
            body: ConfluxTransactionBody {
                from,
                to,
                nonce,
                value: value.unwrap_or_default(),
                data: input.unwrap_or_default(),
                chain_id,
                variant,
            },
            gas_limit,
        })
    }
}

#[derive(Clone, Copy)]
enum ConfluxSpaceContext<'a> {
    Espace {
        engine: &'a ConfluxEngine,
        context: &'a ResolvedEspaceContext,
    },
    CoreSpace {
        engine: &'a ConfluxEngine,
        context: &'a ResolvedCoreSpaceContext,
    },
}

impl ConfluxSpaceContext<'_> {
    async fn nonce(self, address: Address) -> Result<U256, conflux_engine::ConfluxEngineError> {
        match self {
            Self::Espace { engine, context } => engine.espace_nonce(context, address).await,
            Self::CoreSpace { engine, context } => engine.core_space_nonce(context, address).await,
        }
    }

    async fn gas_price(self) -> Result<U256, conflux_engine::ConfluxEngineError> {
        match self {
            Self::Espace { engine, .. } => engine.eth_gas_price().await,
            Self::CoreSpace { engine, .. } => engine.cfx_gas_price().await,
        }
    }

    async fn max_priority_fee_per_gas(self) -> Result<U256, conflux_engine::ConfluxEngineError> {
        match self {
            Self::Espace { engine, .. } => engine.eth_max_priority_fee_per_gas().await,
            Self::CoreSpace { engine, .. } => engine.cfx_max_priority_fee_per_gas().await,
        }
    }

    fn base_fee_per_gas(self) -> Option<U256> {
        match self {
            Self::Espace { context, .. } => context.base_fee_per_gas(),
            Self::CoreSpace { context, .. } => context.base_fee_per_gas(),
        }
    }
}

async fn resolve_fees(
    context: ConfluxSpaceContext<'_>,
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
    context: ConfluxSpaceContext<'_>,
    gas_price: Option<U256>,
) -> Result<U256, conflux_engine::ConfluxEngineError> {
    match gas_price {
        Some(value) => Ok(value),
        None => context.gas_price().await,
    }
}

fn suggested_dynamic_fee_cap(
    context: ConfluxSpaceContext<'_>,
    max_priority_fee_per_gas: U256,
) -> Result<U256, ConfluxServiceError> {
    let base_fee = context.base_fee_per_gas().ok_or_else(|| {
        ConfluxServiceError::transaction_resolution(
            "resolved execution context does not provide a base fee for dynamic fee resolution",
        )
    })?;
    let (base_fee, multiplication_overflow) = base_fee.overflowing_mul(2.into());
    let (max_fee_per_gas, addition_overflow) = base_fee.overflowing_add(max_priority_fee_per_gas);

    if multiplication_overflow || addition_overflow {
        return Err(ConfluxServiceError::transaction_resolution(
            "resolved dynamic fee exceeds an unsigned 256-bit integer",
        ));
    }

    Ok(max_fee_per_gas)
}

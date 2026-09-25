use super::{
    CoreSpaceContext, CoreSpaceSimulationError, CoreSpaceTransactionCompletionError,
    CoreSpaceTransactionInput, CoreSpaceTransactionRejection, CoreSpaceTransactionRequest,
    CoreSpaceTransactionType, CoreSpaceTypedTransaction,
};
use crate::{primitive::u256_from_cfx, state::ConfluxSimulationProvider};
use alloy_primitives::U256;
use conflux_provider::{CoreAccessListItem, CoreTransactionType, EstimateGasAndCollateralRequest};
use simulation_core::{
    completion::{Completion, FeeSource, complete_dynamic_fees, complete_gas_price},
    transaction::TransactionCommon,
};

pub(crate) async fn complete_transaction(
    input: CoreSpaceTransactionInput,
    provider: &ConfluxSimulationProvider,
    context: &CoreSpaceContext,
    chain_id: u32,
    rules: crate::chain_spec::CoreSpaceTransactionValidationRules,
) -> Result<
    Completion<
        CoreSpaceTypedTransaction,
        CoreSpaceTransactionRequest,
        CoreSpaceTransactionRejection,
    >,
    CoreSpaceSimulationError,
> {
    let transaction_type = match &input {
        CoreSpaceTransactionInput::Complete(tx) => tx.transaction_type(),
        CoreSpaceTransactionInput::Partial(tx) => tx.transaction_type()?,
    };
    if let Some(rejection) = reject_transaction(input.as_ref(), transaction_type, chain_id, rules) {
        return Ok(Completion::Rejected {
            transaction: input,
            rejection,
        });
    }
    let mut partial = match input {
        CoreSpaceTransactionInput::Complete(tx) => return Ok(Completion::Ready(tx)),
        CoreSpaceTransactionInput::Partial(tx) => tx,
    };
    let source = CoreFeeSource { provider, context };
    let dynamic_fees = match transaction_type {
        CoreSpaceTransactionType::Cip155 | CoreSpaceTransactionType::Cip2930 => {
            partial.fees.gas_price =
                Some(complete_gas_price(&source, partial.fees.gas_price).await?);
            None
        }
        CoreSpaceTransactionType::Cip1559 => {
            let fees = complete_dynamic_fees(&source, &partial.fees).await?;
            partial.fees.max_fee_per_gas = Some(fees.max_fee_per_gas);
            partial.fees.max_priority_fee_per_gas = Some(fees.max_priority_fee_per_gas);
            Some(fees)
        }
    };
    if let Some(rejection) = reject_transaction(
        simulation_core::transaction::TransactionInput::Partial(&partial),
        transaction_type,
        chain_id,
        rules,
    ) {
        return Ok(Completion::Rejected {
            transaction: CoreSpaceTransactionInput::Partial(partial),
            rejection,
        });
    }
    let nonce = match partial.common.nonce {
        Some(value) => value,
        None => u256_from_cfx(
            provider
                .cfx_get_next_nonce(partial.common.from, context.state_pivot())
                .await
                .map_err(CoreSpaceTransactionCompletionError::from)?,
        ),
    };
    let chain_id = partial.common.chain_id.unwrap_or(chain_id);
    let epoch_height = partial
        .epoch_height
        .unwrap_or_else(|| context.state_anchor.epoch_number());
    let value = partial.common.value.unwrap_or_default();
    let input = partial.common.input.unwrap_or_default();
    let access_list = partial.access_list.unwrap_or_default();
    let (gas_limit, storage_limit) = match (partial.common.gas_limit, partial.storage_limit) {
        (Some(gas_limit), Some(storage_limit)) => (gas_limit, storage_limit),
        (gas_limit, storage_limit) => {
            let mut request = EstimateGasAndCollateralRequest {
                from: partial.common.from,
                to: partial.common.to,
                nonce,
                chain_id: U256::from(chain_id),
                gas: gas_limit,
                storage_limit: storage_limit.map(U256::from),
                epoch_height: Some(U256::from(epoch_height)),
                value,
                data: input.clone(),
                gas_price: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                access_list: None,
                transaction_type: CoreTransactionType::Legacy,
            };
            let copy_access_list = |items: &[super::CoreSpaceAccessListItem]| {
                items
                    .iter()
                    .map(|item| CoreAccessListItem {
                        address: item.address,
                        storage_keys: item.storage_keys.clone(),
                    })
                    .collect()
            };
            match transaction_type {
                CoreSpaceTransactionType::Cip155 => {
                    request.gas_price = partial.fees.gas_price;
                }
                CoreSpaceTransactionType::Cip2930 => {
                    request.gas_price = partial.fees.gas_price;
                    request.access_list = Some(copy_access_list(&access_list));
                    request.transaction_type = CoreTransactionType::AccessList;
                }
                CoreSpaceTransactionType::Cip1559 => {
                    request.max_fee_per_gas = partial.fees.max_fee_per_gas;
                    request.max_priority_fee_per_gas = partial.fees.max_priority_fee_per_gas;
                    request.access_list = Some(copy_access_list(&access_list));
                    request.transaction_type = CoreTransactionType::DynamicFee;
                }
            }
            let estimate = provider
                .cfx_estimate_gas_and_collateral(request, context.state_anchor.core_space_epoch())
                .await
                .map_err(|source| {
                    CoreSpaceTransactionCompletionError::GasAndCollateralEstimation { source }
                })?;
            let gas_limit = gas_limit.unwrap_or(estimate.gas_limit);
            let storage_limit = match storage_limit {
                Some(value) => value,
                None => u64::try_from(estimate.storage_limit).map_err(|_| {
                    CoreSpaceTransactionCompletionError::StorageLimitOutOfRange {
                        value: estimate.storage_limit,
                    }
                })?,
            };
            (gas_limit, storage_limit)
        }
    };
    let common = TransactionCommon {
        from: partial.common.from,
        to: partial.common.to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
    };
    let transaction = match transaction_type {
        CoreSpaceTransactionType::Cip155 => CoreSpaceTypedTransaction::Cip155 {
            common,
            storage_limit,
            epoch_height,
            gas_price: partial
                .fees
                .gas_price
                .expect("fixed gas price was completed"),
        },
        CoreSpaceTransactionType::Cip2930 => CoreSpaceTypedTransaction::Cip2930 {
            common,
            storage_limit,
            epoch_height,
            gas_price: partial
                .fees
                .gas_price
                .expect("fixed gas price was completed"),
            access_list,
        },
        CoreSpaceTransactionType::Cip1559 => CoreSpaceTypedTransaction::Cip1559 {
            common,
            storage_limit,
            epoch_height,
            fees: dynamic_fees.expect("dynamic fees were completed"),
            access_list,
        },
    };
    Ok(Completion::Ready(transaction))
}

struct CoreFeeSource<'a> {
    provider: &'a ConfluxSimulationProvider,
    context: &'a CoreSpaceContext,
}
impl FeeSource for CoreFeeSource<'_> {
    type Error = CoreSpaceTransactionCompletionError;
    fn base_fee(&self) -> Result<U256, Self::Error> {
        Ok(u256_from_cfx(self.context.base_fee_per_gas()))
    }
    async fn gas_price(&self) -> Result<U256, Self::Error> {
        Ok(u256_from_cfx(self.provider.cfx_gas_price().await?))
    }
    async fn priority_fee(&self) -> Result<U256, Self::Error> {
        Ok(u256_from_cfx(
            self.provider.cfx_max_priority_fee_per_gas().await?,
        ))
    }
    fn max_fee_overflow(&self) -> Self::Error {
        Self::Error::MaxFeePerGasOverflow
    }
}

fn reject_transaction(
    transaction: simulation_core::transaction::TransactionInput<
        &CoreSpaceTypedTransaction,
        &CoreSpaceTransactionRequest,
    >,
    transaction_type: CoreSpaceTransactionType,
    expected_chain_id: u32,
    rules: crate::chain_spec::CoreSpaceTransactionValidationRules,
) -> Option<CoreSpaceTransactionRejection> {
    use CoreSpaceTransactionRejection as Rejection;
    use simulation_core::transaction::TransactionInput;
    let (chain_id, cap, priority) = match transaction {
        TransactionInput::Complete(tx) => (
            Some(tx.common().chain_id),
            Some(tx.gas_price_for_sponsorship_check()),
            match tx {
                CoreSpaceTypedTransaction::Cip1559 { fees, .. } => {
                    Some(fees.max_priority_fee_per_gas)
                }
                _ => None,
            },
        ),
        TransactionInput::Partial(tx) => (
            tx.common.chain_id,
            tx.fees.gas_price.or(tx.fees.max_fee_per_gas),
            tx.fees.max_priority_fee_per_gas,
        ),
    };
    if let Some(chain_id) = chain_id
        && chain_id != expected_chain_id
    {
        return Some(Rejection::InvalidChainId {
            transaction_chain_id: chain_id,
            expected_chain_id,
        });
    }
    if !rules.typed_transactions_active {
        match transaction_type {
            CoreSpaceTransactionType::Cip2930 => return Some(Rejection::Cip2930NotActivated),
            CoreSpaceTransactionType::Cip1559 => return Some(Rejection::Cip1559NotActivated),
            CoreSpaceTransactionType::Cip155 => {}
        }
    }
    if cap.is_some_and(|value| value.is_zero()) {
        return Some(if transaction_type == CoreSpaceTransactionType::Cip1559 {
            Rejection::ZeroMaxFeePerGas
        } else {
            Rejection::ZeroGasPrice
        });
    }
    if rules.priority_fee_cap_active
        && let (Some(max_fee_per_gas), Some(max_priority_fee_per_gas)) = (cap, priority)
        && max_priority_fee_per_gas > max_fee_per_gas
    {
        return Some(Rejection::PriorityFeeGreaterThanMaxFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        });
    }
    None
}

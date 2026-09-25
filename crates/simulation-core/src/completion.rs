use std::future::Future;

use crate::transaction::{
    DynamicFees, FeeInput, TransactionCommon, TransactionInput, TransactionInputError,
    TransactionRef, TransactionRequest, TxType, TypedTransaction,
};
use alloy_primitives::{Address, U256};

pub enum Completion<T, P, R> {
    Ready(T),
    Rejected {
        transaction: TransactionInput<T, P>,
        rejection: R,
    },
}

pub trait FeeSource: Sync {
    type Error;
    fn base_fee(&self) -> Result<U256, Self::Error>;
    fn gas_price(&self) -> impl Future<Output = Result<U256, Self::Error>> + Send;
    fn priority_fee(&self) -> impl Future<Output = Result<U256, Self::Error>> + Send;
    fn max_fee_overflow(&self) -> Self::Error;
    fn max_fee_per_gas_limit(&self) -> U256 {
        U256::MAX
    }
}

pub async fn complete_gas_price<S: FeeSource>(
    source: &S,
    value: Option<U256>,
) -> Result<U256, S::Error> {
    match value {
        Some(value) => Ok(value),
        None => source.gas_price().await,
    }
}

pub async fn complete_dynamic_fees<S: FeeSource>(
    source: &S,
    input: &FeeInput,
) -> Result<DynamicFees, S::Error> {
    let max_priority_fee_per_gas = match input.max_priority_fee_per_gas {
        Some(value) => value,
        None => source.priority_fee().await?,
    };
    let max_fee_per_gas = match input.max_fee_per_gas {
        Some(value) => value,
        None => {
            let base = source.base_fee()?;
            base.checked_mul(U256::from(2))
                .and_then(|value| value.checked_add(max_priority_fee_per_gas))
                .filter(|value| *value <= source.max_fee_per_gas_limit())
                .ok_or_else(|| source.max_fee_overflow())?
        }
    };
    Ok(DynamicFees {
        max_fee_per_gas,
        max_priority_fee_per_gas,
    })
}

pub trait TransactionCompletionSource: FeeSource {
    type Rejection;
    fn chain_id(&self) -> u64;
    fn default_type(&self) -> TxType;
    fn check_input(
        &self,
        transaction: TransactionRef<'_>,
        transaction_type: TxType,
    ) -> Result<(), Self::Error>;
    fn rejection(
        &self,
        transaction: TransactionRef<'_>,
        transaction_type: TxType,
    ) -> Result<Option<Self::Rejection>, Self::Error>;
    fn nonce(&self, from: Address) -> impl Future<Output = Result<u64, Self::Error>> + Send;
    fn estimate_gas(
        &self,
        transaction: &TransactionRequest,
    ) -> impl Future<Output = Result<u64, Self::Error>> + Send;
    fn blob_fee(&self) -> impl Future<Output = Result<U256, Self::Error>> + Send;
}

pub async fn complete_transaction<S>(
    input: TransactionInput,
    source: &S,
) -> Result<Completion<TypedTransaction, TransactionRequest, S::Rejection>, S::Error>
where
    S: TransactionCompletionSource,
    S::Error: From<TransactionInputError>,
{
    let transaction_type = match &input {
        TransactionInput::Complete(transaction) => {
            transaction.check_type_requirements()?;
            transaction.transaction_type()
        }
        TransactionInput::Partial(partial) => partial.transaction_type(source.default_type())?,
    };
    source.check_input(input.as_ref(), transaction_type)?;
    if let Some(rejection) = source.rejection(input.as_ref(), transaction_type)? {
        return Ok(Completion::Rejected {
            transaction: input,
            rejection,
        });
    }
    let mut partial = match input {
        TransactionInput::Complete(transaction) => return Ok(Completion::Ready(transaction)),
        TransactionInput::Partial(partial) => partial,
    };
    let dynamic_fees = match transaction_type {
        TxType::Legacy | TxType::Eip2930 => {
            partial.fees.gas_price =
                Some(complete_gas_price(source, partial.fees.gas_price).await?);
            None
        }
        TxType::Eip1559 | TxType::Eip4844 | TxType::Eip7702 => {
            let fees = complete_dynamic_fees(source, &partial.fees).await?;
            partial.fees.max_fee_per_gas = Some(fees.max_fee_per_gas);
            partial.fees.max_priority_fee_per_gas = Some(fees.max_priority_fee_per_gas);
            Some(fees)
        }
    };
    if let Some(rejection) =
        source.rejection(TransactionInput::Partial(&partial), transaction_type)?
    {
        return Ok(Completion::Rejected {
            transaction: TransactionInput::Partial(partial),
            rejection,
        });
    }
    partial.transaction_type = Some(transaction_type);
    if transaction_type != TxType::Legacy {
        partial.access_list.get_or_insert_default();
    }
    if transaction_type == TxType::Eip4844 {
        partial.max_fee_per_blob_gas =
            Some(complete_blob_fee(source, partial.max_fee_per_blob_gas).await?);
    }
    let nonce = match partial.common.nonce {
        Some(nonce) => nonce,
        None => source.nonce(partial.common.from).await?,
    };
    let chain_id = partial.common.chain_id.unwrap_or_else(|| source.chain_id());
    partial.common.nonce = Some(nonce);
    partial.common.chain_id = Some(chain_id);
    let gas_limit = match partial.common.gas_limit {
        Some(value) => value,
        None => source.estimate_gas(&partial).await?,
    };
    let common = TransactionCommon {
        from: partial.common.from,
        to: partial.common.to,
        nonce,
        gas_limit,
        value: partial.common.value.unwrap_or_default(),
        input: partial.common.input.unwrap_or_default(),
        chain_id,
    };
    let access_list = partial.access_list.unwrap_or_default();
    let transaction = match transaction_type {
        TxType::Legacy => TypedTransaction::Legacy {
            common,
            gas_price: partial
                .fees
                .gas_price
                .expect("fixed gas price was completed"),
        },
        TxType::Eip2930 => TypedTransaction::Eip2930 {
            common,
            gas_price: partial
                .fees
                .gas_price
                .expect("fixed gas price was completed"),
            access_list,
        },
        TxType::Eip1559 => TypedTransaction::Eip1559 {
            common,
            fees: dynamic_fees.expect("dynamic fees were completed"),
            access_list,
        },
        TxType::Eip4844 => TypedTransaction::Eip4844 {
            common,
            fees: dynamic_fees.expect("dynamic fees were completed"),
            max_fee_per_blob_gas: partial
                .max_fee_per_blob_gas
                .expect("blob fee was completed"),
            access_list,
            blob_versioned_hashes: partial.blob_versioned_hashes.unwrap_or_default(),
        },
        TxType::Eip7702 => TypedTransaction::Eip7702 {
            common,
            fees: dynamic_fees.expect("dynamic fees were completed"),
            access_list,
            authorization_list: partial.authorization_list.unwrap_or_default(),
        },
    };
    Ok(Completion::Ready(transaction))
}

async fn complete_blob_fee<S: TransactionCompletionSource>(
    source: &S,
    value: Option<U256>,
) -> Result<U256, S::Error> {
    match value {
        Some(value) => Ok(value),
        None => source.blob_fee().await,
    }
}

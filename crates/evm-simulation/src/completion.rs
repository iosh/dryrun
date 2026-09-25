use alloy::primitives::U256;
use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    eips::BlockId,
    network::Ethereum,
    providers::{DynProvider, Provider, layers::BlockIdProvider},
    rpc::types::{
        AccessList as RpcAccessList, TransactionInput as RpcTransactionInput,
        TransactionRequest as RpcTransactionRequest,
    },
};

use crate::{
    DynamicFees, EthereumChainSpec, EvmNotReadyError, EvmSimulationError,
    EvmTransactionCompletionError, FeeInput, PartialTransactionCommon, TransactionCommon,
    TransactionInput, TransactionRequest, TxType, TypedTransaction,
};

pub(crate) async fn complete_transaction(
    input: TransactionInput,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<TypedTransaction, EvmSimulationError> {
    match input {
        TransactionInput::Complete(transaction) => {
            transaction.check_type_requirements()?;
            Ok(transaction)
        }
        TransactionInput::Partial(transaction) => {
            complete_partial_transaction(transaction, provider, block, chain_spec).await
        }
    }
}

async fn complete_partial_transaction(
    transaction: TransactionRequest,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<TypedTransaction, EvmSimulationError> {
    let default_type = if block.base_fee_per_gas().is_some() {
        TxType::Eip1559
    } else {
        TxType::Legacy
    };
    let transaction_type = transaction.transaction_type(default_type)?;
    let TransactionRequest {
        common,
        fees,
        transaction_type: _,
        max_fee_per_blob_gas,
        access_list,
        blob_versioned_hashes,
        authorization_list,
    } = transaction;
    let PartialTransactionCommon {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
    } = common;
    let FeeInput {
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
    } = fees;
    let block_id = BlockId::hash_canonical(block.hash());
    let anchored_provider = BlockIdProvider::new(provider.clone(), block_id);
    let nonce = match nonce {
        Some(nonce) => nonce,
        None => anchored_provider
            .get_transaction_count(from)
            .await
            .map_err(|source| EvmTransactionCompletionError::NonceLookup {
                block_number: block.number(),
                source,
            })?,
    };
    let value = value.unwrap_or_default();
    let input = input.unwrap_or_default();
    let estimation_chain_id = chain_spec.chain_id();
    let chain_id = chain_id.unwrap_or(estimation_chain_id);
    let needs_gas_estimate = gas_limit.is_none();
    let common = TransactionCommon {
        from,
        to,
        nonce,
        gas_limit: gas_limit.unwrap_or_default(),
        value,
        input,
        chain_id,
    };
    let access_list = access_list.unwrap_or_default();
    let blob_versioned_hashes = blob_versioned_hashes.unwrap_or_default();
    let authorization_list = authorization_list.unwrap_or_default();
    let mut transaction = match transaction_type {
        TxType::Legacy => TypedTransaction::Legacy {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
        },
        TxType::Eip2930 => TypedTransaction::Eip2930 {
            common,
            gas_price: complete_gas_price(provider, gas_price).await?,
            access_list,
        },
        TxType::Eip1559 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            TypedTransaction::Eip1559 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
                access_list,
            }
        }
        TxType::Eip4844 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            let max_fee_per_blob_gas = match max_fee_per_blob_gas {
                Some(value) => value,
                None => {
                    let params = chain_spec
                        .execution_spec(block.number(), block.timestamp())
                        .map_err(EvmNotReadyError::from)?
                        .blob_params
                        .ok_or(EvmTransactionCompletionError::MissingBlobBaseFee {
                            block_number: block.number(),
                        })?;
                    let excess = block.excess_blob_gas().ok_or(
                        EvmTransactionCompletionError::MissingBlobBaseFee {
                            block_number: block.number(),
                        },
                    )?;
                    U256::from(params.calc_blob_fee(excess))
                }
            };
            TypedTransaction::Eip4844 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
                max_fee_per_blob_gas,
                access_list,
                blob_versioned_hashes,
            }
        }
        TxType::Eip7702 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            TypedTransaction::Eip7702 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
                access_list,
                authorization_list,
            }
        }
    };

    if needs_gas_estimate {
        let mut request = gas_estimation_request(&transaction)?;
        request.chain_id = Some(estimation_chain_id);
        let gas_limit = anchored_provider
            .estimate_gas(request)
            .await
            .map_err(|source| EvmTransactionCompletionError::GasEstimation {
                block_number: block.number(),
                source,
            })?;
        transaction.common_mut().gas_limit = gas_limit;
    }

    Ok(transaction)
}

async fn complete_gas_price(
    provider: &DynProvider<Ethereum>,
    gas_price: Option<U256>,
) -> Result<U256, EvmTransactionCompletionError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider
            .get_gas_price()
            .await
            .map(U256::from)
            .map_err(|source| EvmTransactionCompletionError::GasPriceSuggestion { source }),
    }
}

async fn complete_dynamic_fees(
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
) -> Result<(U256, U256), EvmTransactionCompletionError> {
    let max_priority_fee_per_gas = match max_priority_fee_per_gas {
        Some(value) => value,
        None => provider
            .get_max_priority_fee_per_gas()
            .await
            .map(U256::from)
            .map_err(|source| EvmTransactionCompletionError::PriorityFeeSuggestion { source })?,
    };
    let max_fee_per_gas = match max_fee_per_gas {
        Some(value) => value,
        None => suggested_max_fee_per_gas(block.inner(), max_priority_fee_per_gas)?,
    };

    Ok((max_fee_per_gas, max_priority_fee_per_gas))
}

fn suggested_max_fee_per_gas(
    block: &Header,
    max_priority_fee_per_gas: U256,
) -> Result<U256, EvmTransactionCompletionError> {
    let base_fee =
        block
            .base_fee_per_gas()
            .ok_or(EvmTransactionCompletionError::MissingBaseFee {
                block_number: block.number(),
            })?;

    U256::from(base_fee)
        .checked_mul(U256::from(2))
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or(EvmTransactionCompletionError::MaxFeePerGasOverflow)
}

fn gas_estimation_request(
    transaction: &TypedTransaction,
) -> Result<RpcTransactionRequest, crate::TransactionInputError> {
    let common = transaction.common();
    let mut request = RpcTransactionRequest {
        from: Some(common.from),
        to: Some(common.to.map_or(
            alloy::primitives::TxKind::Create,
            alloy::primitives::TxKind::Call,
        )),
        value: Some(common.value),
        input: RpcTransactionInput::new(common.input.clone()),
        nonce: Some(common.nonce),
        chain_id: Some(common.chain_id),
        ..Default::default()
    };

    request.transaction_type = Some(transaction.transaction_type() as u8);
    if let Some(fees) = transaction.dynamic_fees() {
        request.max_fee_per_gas = Some(checked_fee("maxFeePerGas", fees.max_fee_per_gas)?);
        request.max_priority_fee_per_gas = Some(checked_fee(
            "maxPriorityFeePerGas",
            fees.max_priority_fee_per_gas,
        )?);
    } else {
        request.gas_price = Some(checked_fee("gasPrice", transaction.gas_price_cap())?);
    }
    if transaction.transaction_type() != TxType::Legacy {
        request.access_list = Some(RpcAccessList(
            transaction
                .access_list()
                .iter()
                .map(|item| alloy::eips::eip2930::AccessListItem {
                    address: item.address,
                    storage_keys: item.storage_keys.clone(),
                })
                .collect(),
        ));
    }
    match transaction {
        TypedTransaction::Eip4844 {
            max_fee_per_blob_gas,
            blob_versioned_hashes,
            ..
        } => {
            request.max_fee_per_blob_gas =
                Some(checked_fee("maxFeePerBlobGas", *max_fee_per_blob_gas)?);
            request.blob_versioned_hashes = Some(blob_versioned_hashes.clone());
        }
        TypedTransaction::Eip7702 {
            authorization_list, ..
        } => {
            request.authorization_list = Some(authorization_list.clone());
        }
        _ => {}
    }

    Ok(request)
}

fn checked_fee(field: &'static str, value: U256) -> Result<u128, crate::TransactionInputError> {
    u128::try_from(value).map_err(|_| crate::TransactionInputError::OutOfRange {
        field,
        value,
        maximum: U256::from(u128::MAX),
    })
}

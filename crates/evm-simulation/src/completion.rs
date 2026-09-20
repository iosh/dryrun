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
    DynamicFees, EthereumChainSpec, EvmTransactionCompletionError, PartialTransaction,
    TransactionCommon, TransactionInput, TxType, TypedTransaction,
};

pub(crate) async fn complete_transaction(
    input: TransactionInput,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<TypedTransaction, EvmTransactionCompletionError> {
    match input {
        TransactionInput::Complete(transaction) => {
            transaction.validate_fields()?;
            Ok(transaction)
        }
        TransactionInput::Partial(transaction) => {
            complete_partial_transaction(transaction, provider, block, chain_spec).await
        }
    }
}

async fn complete_partial_transaction(
    transaction: PartialTransaction,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<TypedTransaction, EvmTransactionCompletionError> {
    let transaction_type = transaction
        .transaction_type
        .unwrap_or_else(|| transaction.preferred_type());
    transaction.validate(transaction_type)?;

    let PartialTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
        transaction_type: _,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        max_fee_per_blob_gas,
        access_list,
        blob_versioned_hashes,
        authorization_list,
    } = transaction;
    let block_id = BlockId::Hash(block.hash().into());
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
            gas_price: U256::from(complete_gas_price(provider, gas_price).await?),
        },
        TxType::Eip2930 => TypedTransaction::Eip2930 {
            common,
            gas_price: U256::from(complete_gas_price(provider, gas_price).await?),
            access_list,
        },
        TxType::Eip1559 => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            TypedTransaction::Eip1559 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas: U256::from(max_fee_per_gas),
                    max_priority_fee_per_gas: U256::from(max_priority_fee_per_gas),
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
                None => provider.get_blob_base_fee().await.map_err(|source| {
                    EvmTransactionCompletionError::BlobBaseFeeLookup { source }
                })?,
            };
            TypedTransaction::Eip4844 {
                common,
                fees: DynamicFees {
                    max_fee_per_gas: U256::from(max_fee_per_gas),
                    max_priority_fee_per_gas: U256::from(max_priority_fee_per_gas),
                },
                max_fee_per_blob_gas: U256::from(max_fee_per_blob_gas),
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
                    max_fee_per_gas: U256::from(max_fee_per_gas),
                    max_priority_fee_per_gas: U256::from(max_priority_fee_per_gas),
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
    gas_price: Option<u128>,
) -> Result<u128, EvmTransactionCompletionError> {
    match gas_price {
        Some(value) => Ok(value),
        None => provider
            .get_gas_price()
            .await
            .map_err(|source| EvmTransactionCompletionError::GasPriceSuggestion { source }),
    }
}

async fn complete_dynamic_fees(
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    max_fee_per_gas: Option<u128>,
    max_priority_fee_per_gas: Option<u128>,
) -> Result<(u128, u128), EvmTransactionCompletionError> {
    let max_priority_fee_per_gas = match max_priority_fee_per_gas {
        Some(value) => value,
        None => provider
            .get_max_priority_fee_per_gas()
            .await
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
    max_priority_fee_per_gas: u128,
) -> Result<u128, EvmTransactionCompletionError> {
    let base_fee =
        block
            .base_fee_per_gas()
            .ok_or(EvmTransactionCompletionError::MissingBaseFee {
                block_number: block.number(),
            })?;

    u128::from(base_fee)
        .checked_mul(2)
        .and_then(|value| value.checked_add(max_priority_fee_per_gas))
        .ok_or(EvmTransactionCompletionError::MaxFeePerGasOverflow)
}

fn gas_estimation_request(
    transaction: &TypedTransaction,
) -> Result<RpcTransactionRequest, crate::TransactionInputError> {
    use crate::transaction::checked_fee;
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

#[cfg(test)]
mod tests {
    use std::future::Future;

    use alloy::{
        consensus::{Header, Sealed},
        network::Ethereum,
        primitives::{Address, B256, Bytes, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::client::RpcClient,
        transports::mock::Asserter,
    };

    use super::{complete_transaction, suggested_max_fee_per_gas};
    use crate::{
        EthereumChainSpec, EvmTransactionCompletionError, PartialTransaction, TransactionInput,
        TxType, TypedTransaction,
    };

    #[test]
    fn completes_missing_nonce_gas_and_gas_price() {
        let asserter = Asserter::new();
        asserter.push_success(&"0x7");
        asserter.push_success(&"0x64");
        asserter.push_success(&"0x5208");
        let provider = mock_provider(asserter.clone());
        let block = block(42, B256::repeat_byte(9), Some(10));

        let completed = block_on(complete_transaction(
            TransactionInput::Partial(PartialTransaction {
                from: Address::repeat_byte(1),
                to: Some(Address::repeat_byte(2)),
                nonce: None,
                gas_limit: None,
                value: None,
                input: None,
                chain_id: None,
                transaction_type: Some(TxType::Legacy),
                gas_price: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                max_fee_per_blob_gas: None,
                access_list: None,
                blob_versioned_hashes: None,
                authorization_list: None,
            }),
            &provider,
            &block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("missing provider-backed fields should complete");

        assert_eq!(completed.common().nonce, 7);
        assert_eq!(completed.common().gas_limit, 21_000);
        assert!(matches!(
            completed,
            TypedTransaction::Legacy { gas_price, .. } if gas_price == U256::from(100)
        ));
        assert!(asserter.read_q().is_empty());
    }

    #[test]
    fn completes_dynamic_and_blob_fee_suggestions() {
        let from = Address::repeat_byte(1);
        let to = Address::repeat_byte(2);
        let dynamic_asserter = Asserter::new();
        dynamic_asserter.push_success(&"0x3");
        let dynamic_provider = mock_provider(dynamic_asserter.clone());
        let dynamic_block = block(42, B256::repeat_byte(5), Some(10));

        let dynamic = block_on(complete_transaction(
            TransactionInput::Partial(PartialTransaction {
                from,
                to: Some(to),
                nonce: Some(6),
                gas_limit: Some(7),
                value: Some(U256::from(8)),
                input: Some(Bytes::from_static(&[3, 4])),
                chain_id: Some(5),
                transaction_type: Some(TxType::Eip1559),
                gas_price: None,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                max_fee_per_blob_gas: None,
                access_list: Some(Vec::new()),
                blob_versioned_hashes: None,
                authorization_list: None,
            }),
            &dynamic_provider,
            &dynamic_block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("dynamic fees should complete");

        assert!(matches!(
            dynamic,
            TypedTransaction::Eip1559 { fees, .. }
                if fees.max_fee_per_gas == U256::from(23)
                    && fees.max_priority_fee_per_gas == U256::from(3)
        ));
        assert!(dynamic_asserter.read_q().is_empty());

        let blob_asserter = Asserter::new();
        blob_asserter.push_success(&"0x20");
        let blob_provider = mock_provider(blob_asserter.clone());
        let blob_block = block(43, B256::repeat_byte(7), Some(11));
        let blob = block_on(complete_transaction(
            TransactionInput::Partial(PartialTransaction {
                from,
                to: Some(to),
                nonce: Some(9),
                gas_limit: Some(10),
                value: None,
                input: None,
                chain_id: None,
                transaction_type: Some(TxType::Eip4844),
                gas_price: None,
                max_fee_per_gas: Some(30),
                max_priority_fee_per_gas: Some(2),
                max_fee_per_blob_gas: None,
                access_list: Some(Vec::new()),
                blob_versioned_hashes: Some(vec![B256::repeat_byte(6)]),
                authorization_list: None,
            }),
            &blob_provider,
            &blob_block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("blob fee should complete");

        assert!(matches!(
            blob,
            TypedTransaction::Eip4844 { max_fee_per_blob_gas, .. }
                if max_fee_per_blob_gas == U256::from(32)
        ));
        assert!(blob_asserter.read_q().is_empty());
    }

    #[test]
    fn reports_typed_dynamic_fee_failures() {
        let missing_base_fee = Header {
            number: 12,
            base_fee_per_gas: None,
            ..Default::default()
        };
        assert!(matches!(
            suggested_max_fee_per_gas(&missing_base_fee, 1),
            Err(EvmTransactionCompletionError::MissingBaseFee { block_number: 12 })
        ));

        let overflowing = Header {
            base_fee_per_gas: Some(u64::MAX),
            ..Default::default()
        };
        assert!(matches!(
            suggested_max_fee_per_gas(&overflowing, u128::MAX),
            Err(EvmTransactionCompletionError::MaxFeePerGasOverflow)
        ));
    }

    fn block(number: u64, hash: B256, base_fee_per_gas: Option<u64>) -> Sealed<Header> {
        Sealed::new_unchecked(
            Header {
                number,
                base_fee_per_gas,
                ..Default::default()
            },
            hash,
        )
    }

    fn mock_provider(asserter: Asserter) -> DynProvider<Ethereum> {
        RootProvider::new(RpcClient::mocked(asserter)).erased()
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build")
            .block_on(future)
    }
}

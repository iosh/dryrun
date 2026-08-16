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
    CompleteTransaction, CompleteTransactionVariant, EthereumChainSpec,
    EvmTransactionCompletionError, PartialTransaction, PartialTransactionVariant, TransactionInput,
};

pub(crate) async fn complete_transaction(
    input: TransactionInput,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
    chain_spec: &EthereumChainSpec,
) -> Result<CompleteTransaction, EvmTransactionCompletionError> {
    match input {
        TransactionInput::Complete(transaction) => Ok(transaction),
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
) -> Result<CompleteTransaction, EvmTransactionCompletionError> {
    let PartialTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
        variant,
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
    let variant = complete_variant(variant, provider, block).await?;
    let value = value.unwrap_or_default();
    let input = input.unwrap_or_default();
    let estimation_chain_id = chain_spec.chain_id();
    let chain_id = chain_id.unwrap_or(estimation_chain_id);
    let gas_limit = match gas_limit {
        Some(gas_limit) => gas_limit,
        None => anchored_provider
            .estimate_gas(gas_estimation_request(
                from,
                to,
                nonce,
                value,
                input.clone(),
                estimation_chain_id,
                &variant,
            ))
            .await
            .map_err(|source| EvmTransactionCompletionError::GasEstimation {
                block_number: block.number(),
                source,
            })?,
    };

    Ok(CompleteTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value,
        input,
        chain_id,
        variant,
    })
}

async fn complete_variant(
    variant: PartialTransactionVariant,
    provider: &DynProvider<Ethereum>,
    block: &Sealed<Header>,
) -> Result<CompleteTransactionVariant, EvmTransactionCompletionError> {
    match variant {
        PartialTransactionVariant::Legacy { gas_price } => Ok(CompleteTransactionVariant::Legacy {
            gas_price: complete_gas_price(provider, gas_price).await?,
        }),
        PartialTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => Ok(CompleteTransactionVariant::Eip2930 {
            gas_price: complete_gas_price(provider, gas_price).await?,
            access_list,
        }),
        PartialTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;

            Ok(CompleteTransactionVariant::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            })
        }
        PartialTransactionVariant::Eip4844 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            max_fee_per_blob_gas,
            access_list,
            blob_versioned_hashes,
        } => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;
            let max_fee_per_blob_gas = match max_fee_per_blob_gas {
                Some(value) => value,
                None => provider.get_blob_base_fee().await.map_err(|source| {
                    EvmTransactionCompletionError::BlobBaseFeeLookup { source }
                })?,
            };

            Ok(CompleteTransactionVariant::Eip4844 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                max_fee_per_blob_gas,
                access_list,
                blob_versioned_hashes,
            })
        }
        PartialTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
            authorization_list,
        } => {
            let (max_fee_per_gas, max_priority_fee_per_gas) =
                complete_dynamic_fees(provider, block, max_fee_per_gas, max_priority_fee_per_gas)
                    .await?;

            Ok(CompleteTransactionVariant::Eip7702 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
                authorization_list,
            })
        }
    }
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
    from: alloy::primitives::Address,
    to: Option<alloy::primitives::Address>,
    nonce: u64,
    value: alloy::primitives::U256,
    input: alloy::primitives::Bytes,
    chain_id: u64,
    variant: &CompleteTransactionVariant,
) -> RpcTransactionRequest {
    let mut request = RpcTransactionRequest {
        from: Some(from),
        to: Some(to.map_or(
            alloy::primitives::TxKind::Create,
            alloy::primitives::TxKind::Call,
        )),
        value: Some(value),
        input: RpcTransactionInput::new(input),
        nonce: Some(nonce),
        chain_id: Some(chain_id),
        ..Default::default()
    };

    match variant {
        CompleteTransactionVariant::Legacy { gas_price } => {
            request.transaction_type = Some(0);
            request.gas_price = Some(*gas_price);
        }
        CompleteTransactionVariant::Eip2930 {
            gas_price,
            access_list,
        } => {
            request.transaction_type = Some(1);
            request.gas_price = Some(*gas_price);
            request.access_list = Some(RpcAccessList(access_list.clone()));
        }
        CompleteTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => {
            request.transaction_type = Some(2);
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.access_list = Some(RpcAccessList(access_list.clone()));
        }
        CompleteTransactionVariant::Eip4844 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            max_fee_per_blob_gas,
            access_list,
            blob_versioned_hashes,
        } => {
            request.transaction_type = Some(3);
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.max_fee_per_blob_gas = Some(*max_fee_per_blob_gas);
            request.access_list = Some(RpcAccessList(access_list.clone()));
            request.blob_versioned_hashes = Some(blob_versioned_hashes.clone());
        }
        CompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
            authorization_list,
        } => {
            request.transaction_type = Some(4);
            request.max_fee_per_gas = Some(*max_fee_per_gas);
            request.max_priority_fee_per_gas = Some(*max_priority_fee_per_gas);
            request.access_list = Some(RpcAccessList(access_list.clone()));
            request.authorization_list = Some(authorization_list.clone());
        }
    }

    request
}

#[cfg(test)]
mod tests {
    use std::future::Future;

    use alloy::{
        consensus::{Header, Sealed},
        network::Ethereum,
        primitives::{Address, B256, Bytes, U256},
        providers::{DynProvider, Provider, RootProvider},
        rpc::{client::RpcClient, types::AccessList as RpcAccessList},
        transports::mock::Asserter,
    };

    use super::{complete_transaction, gas_estimation_request, suggested_max_fee_per_gas};
    use crate::{
        AccessListItem, Authorization, CompleteTransaction, CompleteTransactionVariant,
        EthereumChainSpec, EvmTransactionCompletionError, PartialTransaction,
        PartialTransactionVariant, SignedAuthorization, TransactionInput,
    };

    #[test]
    fn complete_input_is_returned_without_provider_requests() {
        let transaction = CompleteTransaction {
            from: Address::repeat_byte(1),
            to: None,
            nonce: 2,
            gas_limit: 30_000,
            value: U256::from(3),
            input: Bytes::from_static(&[4]),
            chain_id: 5,
            variant: CompleteTransactionVariant::Legacy { gas_price: 6 },
        };
        let asserter = Asserter::new();
        let provider = mock_provider(asserter.clone());
        let block = block(7, B256::repeat_byte(8), Some(9));

        let completed = block_on(complete_transaction(
            TransactionInput::Complete(transaction.clone()),
            &provider,
            &block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("complete input should not require completion");

        assert_eq!(completed, transaction);
        assert!(asserter.read_q().is_empty());
    }

    #[test]
    fn completes_missing_legacy_fields_and_deterministic_defaults() {
        let asserter = Asserter::new();
        asserter.push_success(&"0x7");
        asserter.push_success(&"0x64");
        asserter.push_success(&"0x5208");
        let provider = mock_provider(asserter.clone());
        let block = block(42, B256::repeat_byte(9), Some(10));
        let from = Address::repeat_byte(1);
        let to = Address::repeat_byte(2);

        let completed = block_on(complete_transaction(
            TransactionInput::Partial(PartialTransaction {
                from,
                to: Some(to),
                nonce: None,
                gas_limit: None,
                value: None,
                input: None,
                chain_id: None,
                variant: PartialTransactionVariant::Legacy { gas_price: None },
            }),
            &provider,
            &block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("partial legacy transaction should complete");

        assert_eq!(
            completed,
            CompleteTransaction {
                from,
                to: Some(to),
                nonce: 7,
                gas_limit: 21_000,
                value: U256::ZERO,
                input: Bytes::new(),
                chain_id: 1,
                variant: CompleteTransactionVariant::Legacy { gas_price: 100 },
            }
        );
        assert!(asserter.read_q().is_empty());
    }

    #[test]
    fn completes_dynamic_and_blob_fee_suggestions() {
        let from = Address::repeat_byte(1);
        let to = Address::repeat_byte(2);
        let input = Bytes::from_static(&[3, 4]);
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
                input: Some(input.clone()),
                chain_id: Some(5),
                variant: PartialTransactionVariant::Eip1559 {
                    max_fee_per_gas: None,
                    max_priority_fee_per_gas: None,
                    access_list: Vec::new(),
                },
            }),
            &dynamic_provider,
            &dynamic_block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("dynamic fees should complete");

        assert_eq!(dynamic.chain_id, 5);
        assert_eq!(dynamic.value, U256::from(8));
        assert_eq!(dynamic.input, input);
        assert!(matches!(
            dynamic.variant,
            CompleteTransactionVariant::Eip1559 {
                max_fee_per_gas: 23,
                max_priority_fee_per_gas: 3,
                ..
            }
        ));
        assert!(dynamic_asserter.read_q().is_empty());

        let blob_hashes = vec![B256::repeat_byte(6)];
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
                variant: PartialTransactionVariant::Eip4844 {
                    max_fee_per_gas: Some(30),
                    max_priority_fee_per_gas: Some(2),
                    max_fee_per_blob_gas: None,
                    access_list: Vec::new(),
                    blob_versioned_hashes: blob_hashes.clone(),
                },
            }),
            &blob_provider,
            &blob_block,
            &EthereumChainSpec::mainnet(),
        ))
        .expect("blob fee should complete");

        assert!(matches!(
            blob.variant,
            CompleteTransactionVariant::Eip4844 {
                max_fee_per_gas: 30,
                max_priority_fee_per_gas: 2,
                max_fee_per_blob_gas: 32,
                blob_versioned_hashes,
                ..
            } if blob_versioned_hashes == blob_hashes
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

    #[test]
    fn maps_eip4844_and_eip7702_estimation_fields() {
        let from = Address::repeat_byte(1);
        let to = Address::repeat_byte(2);
        let access_list = vec![AccessListItem {
            address: Address::repeat_byte(3),
            storage_keys: vec![B256::repeat_byte(4)],
        }];
        let blob_hashes = vec![B256::repeat_byte(5)];
        let authorization_list = vec![signed_authorization()];

        let eip4844 = gas_estimation_request(
            from,
            Some(to),
            6,
            U256::from(7),
            Bytes::from_static(&[8]),
            1,
            &CompleteTransactionVariant::Eip4844 {
                max_fee_per_gas: 9,
                max_priority_fee_per_gas: 10,
                max_fee_per_blob_gas: 11,
                access_list: access_list.clone(),
                blob_versioned_hashes: blob_hashes.clone(),
            },
        );
        assert_eq!(eip4844.transaction_type, Some(3));
        assert_eq!(eip4844.max_fee_per_gas, Some(9));
        assert_eq!(eip4844.max_priority_fee_per_gas, Some(10));
        assert_eq!(eip4844.max_fee_per_blob_gas, Some(11));
        assert_eq!(
            eip4844.access_list,
            Some(RpcAccessList(access_list.clone()))
        );
        assert_eq!(eip4844.blob_versioned_hashes, Some(blob_hashes));

        let eip7702 = gas_estimation_request(
            from,
            Some(to),
            6,
            U256::from(7),
            Bytes::from_static(&[8]),
            1,
            &CompleteTransactionVariant::Eip7702 {
                max_fee_per_gas: 12,
                max_priority_fee_per_gas: 13,
                access_list: access_list.clone(),
                authorization_list: authorization_list.clone(),
            },
        );
        assert_eq!(eip7702.transaction_type, Some(4));
        assert_eq!(eip7702.max_fee_per_gas, Some(12));
        assert_eq!(eip7702.max_priority_fee_per_gas, Some(13));
        assert_eq!(eip7702.access_list, Some(RpcAccessList(access_list)));
        assert_eq!(eip7702.authorization_list, Some(authorization_list));
    }

    fn signed_authorization() -> SignedAuthorization {
        SignedAuthorization::new_unchecked(
            Authorization {
                chain_id: U256::from(1),
                address: Address::repeat_byte(11),
                nonce: 12,
            },
            0,
            U256::from(13),
            U256::from(14),
        )
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

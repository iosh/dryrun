use std::convert::TryFrom;

use alloy::primitives::U256;

use crate::{errors::ValidationError, interface as rpc};

use super::shared::parse_u64_quantity;

impl TryFrom<rpc::EvmSimulateTransactionRequest> for evm_service::SimulateEvmTransactionInput {
    type Error = ValidationError;

    fn try_from(request: rpc::EvmSimulateTransactionRequest) -> Result<Self, Self::Error> {
        request.validate()?;

        let rpc::EvmSimulateTransactionRequest {
            block, transaction, ..
        } = request;

        Ok(Self {
            block: block
                .map(map_block_ref)
                .transpose()?
                .unwrap_or(evm_service::BlockSelector::Latest),
            transaction: map_transaction(transaction)?,
        })
    }
}

fn map_block_ref(block: rpc::BlockRef) -> Result<evm_service::BlockSelector, ValidationError> {
    match block {
        rpc::BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(evm_service::BlockSelector::Latest),
            "safe" => Ok(evm_service::BlockSelector::Safe),
            "finalized" => Ok(evm_service::BlockSelector::Finalized),
            value => Ok(evm_service::BlockSelector::Number(parse_u64_quantity(
                value, "block",
            )?)),
        },
        rpc::BlockRef::Hash(_) => Err(ValidationError::not_supported(
            "`block.blockHash` is not supported yet",
        )),
    }
}

fn map_transaction(
    transaction: rpc::Transaction,
) -> Result<evm_service::EvmTransaction, ValidationError> {
    let variant = map_transaction_variant(&transaction)?;

    Ok(evm_service::EvmTransaction {
        chain_id: transaction
            .chain_id
            .ok_or_else(|| ValidationError::invalid_params("`transaction.chainId` is required"))?,
        from: transaction.from,
        to: transaction.to,
        nonce: transaction
            .nonce
            .ok_or_else(|| ValidationError::invalid_params("`transaction.nonce` is required"))?,
        gas_limit: transaction.gas,
        value: transaction.value.unwrap_or(U256::ZERO),
        data: transaction.data.unwrap_or_default(),
        variant,
    })
}

fn infer_transaction_type(transaction: &rpc::Transaction) -> u8 {
    match transaction.tx_type {
        Some(tx_type @ 0x0..=0x2) => tx_type,
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            0x2
        }
        None if transaction.access_list.as_ref().is_some() => 0x1,
        None => 0x0,
        Some(_) => unreachable!("transaction type is already validated"),
    }
}

fn map_transaction_variant(
    transaction: &rpc::Transaction,
) -> Result<evm_service::EvmTransactionVariant, ValidationError> {
    let tx_type = infer_transaction_type(transaction);
    let access_list = transaction
        .access_list
        .as_ref()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(convert_access_list_item)
        .collect();

    match tx_type {
        0x0 => Ok(evm_service::EvmTransactionVariant::Legacy {
            gas_price: transaction.gas_price.ok_or_else(|| {
                ValidationError::invalid_params("`transaction.gasPrice` is required for type `0x0`")
            })?,
        }),
        0x1 => Ok(evm_service::EvmTransactionVariant::Eip2930 {
            gas_price: transaction.gas_price.ok_or_else(|| {
                ValidationError::invalid_params("`transaction.gasPrice` is required for type `0x1`")
            })?,
            access_list,
        }),
        0x2 => Ok(evm_service::EvmTransactionVariant::Eip1559 {
            max_fee_per_gas: transaction.max_fee_per_gas.ok_or_else(|| {
                ValidationError::invalid_params(
                    "`transaction.maxFeePerGas` is required for type `0x2`",
                )
            })?,
            max_priority_fee_per_gas: transaction.max_priority_fee_per_gas.ok_or_else(|| {
                ValidationError::invalid_params(
                    "`transaction.maxPriorityFeePerGas` is required for type `0x2`",
                )
            })?,
            access_list,
        }),
        _ => unreachable!("transaction type is already validated"),
    }
}

fn convert_access_list_item(item: rpc::AccessListItem) -> evm_service::AccessListItem {
    evm_service::AccessListItem {
        address: item.address,
        storage_keys: item.storage_keys,
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use alloy::primitives::{Address, Bytes, U256};
    use serde_json::json;

    use crate::interface as rpc;

    #[test]
    fn request_maps_into_final_service_transaction() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: Some(rpc::BlockRef::Tag("latest".to_string())),
            options: None,
            transaction: sample_transaction(),
        };

        let input: evm_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");
        assert!(matches!(input.block, evm_service::BlockSelector::Latest));
        assert!(matches!(
            input.transaction.variant,
            evm_service::EvmTransactionVariant::Legacy { gas_price: 1 }
        ));
        assert_eq!(input.transaction.chain_id, 1);
        assert_eq!(input.transaction.nonce, 0);
        assert_eq!(input.transaction.value, U256::ZERO);
        assert_eq!(input.transaction.data, Bytes::new());
    }

    #[test]
    fn block_quantity_maps_into_service_input() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: Some(rpc::BlockRef::Tag("0x1234".to_string())),
            options: None,
            transaction: sample_transaction(),
        };

        let input: evm_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");

        assert!(matches!(
            input.block,
            evm_service::BlockSelector::Number(0x1234)
        ));
    }

    #[test]
    fn safe_and_finalized_block_tags_map_into_service_input() {
        for (tag, expected_selector) in [
            ("safe", evm_service::BlockSelector::Safe),
            ("finalized", evm_service::BlockSelector::Finalized),
        ] {
            let request = rpc::EvmSimulateTransactionRequest {
                block: Some(rpc::BlockRef::Tag(tag.to_string())),
                options: None,
                transaction: sample_transaction(),
            };

            let input: evm_service::SimulateEvmTransactionInput =
                request.try_into().expect("request should map");

            assert_eq!(input.block, expected_selector);
        }
    }

    #[test]
    fn reserved_options_are_rejected() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: None,
            options: Some(rpc::SimulateTransactionOptions {
                include: Some(json!(["changes"])),
                ..Default::default()
            }),
            transaction: sample_transaction(),
        };

        let error = evm_service::SimulateEvmTransactionInput::try_from(request)
            .expect_err("reserved options should be rejected");

        assert_eq!(
            error.to_string(),
            "`options.include` is reserved and not supported yet"
        );
    }

    fn sample_transaction() -> rpc::Transaction {
        rpc::Transaction {
            tx_type: None,
            chain_id: Some(1),
            from: Address::from_str("0x1111111111111111111111111111111111111111").unwrap(),
            to: Some(Address::from_str("0x2222222222222222222222222222222222222222").unwrap()),
            nonce: Some(0),
            gas: 0x5208,
            value: None,
            data: None,
            access_list: None,
            gas_price: Some(1),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }
}

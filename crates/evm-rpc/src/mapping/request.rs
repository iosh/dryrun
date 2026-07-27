use std::convert::TryFrom;

use alloy::primitives::U256;
use simulation_transaction::{
    AccessListItem as SimulationAccessListItem, TransactionRequest as SimulationTransactionRequest,
    TransactionType,
};

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
) -> Result<SimulationTransactionRequest, ValidationError> {
    let request = SimulationTransactionRequest {
        from: transaction.from,
        to: transaction.to,
        nonce: transaction.nonce.map(U256::from),
        gas_limit: Some(U256::from(transaction.gas)),
        value: transaction.value,
        input: transaction.data,
        chain_id: transaction.chain_id.map(U256::from),
        transaction_type: map_transaction_type(transaction.tx_type)?,
        access_list: transaction.access_list.map(|items| {
            items
                .into_iter()
                .map(to_simulation_access_list_item)
                .collect()
        }),
        gas_price: transaction.gas_price.map(U256::from),
        max_fee_per_gas: transaction.max_fee_per_gas.map(U256::from),
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas.map(U256::from),
    };

    request
        .validate_shape()
        .map_err(|error| ValidationError::invalid_params(error.to_string()))?;

    Ok(request)
}

fn map_transaction_type(
    transaction_type: Option<u8>,
) -> Result<Option<TransactionType>, ValidationError> {
    transaction_type
        .map(|transaction_type| match transaction_type {
            0x0 => Ok(TransactionType::Legacy),
            0x1 => Ok(TransactionType::AccessList),
            0x2 => Ok(TransactionType::DynamicFee),
            _ => Err(ValidationError::not_supported(
                "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
            )),
        })
        .transpose()
}

fn to_simulation_access_list_item(item: rpc::AccessListItem) -> SimulationAccessListItem {
    SimulationAccessListItem {
        address: item.address,
        storage_keys: item.storage_keys,
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use alloy::primitives::{Address, U256};
    use serde_json::json;

    use crate::interface as rpc;

    #[test]
    fn request_maps_into_partial_service_transaction() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: Some(rpc::BlockRef::Tag("latest".to_string())),
            options: None,
            transaction: sample_transaction(),
        };

        let input: evm_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");
        assert!(matches!(input.block, evm_service::BlockSelector::Latest));
        assert_eq!(
            input.transaction.resolved_type(),
            simulation_transaction::TransactionType::Legacy
        );
        assert_eq!(input.transaction.chain_id, Some(U256::from(1)));
        assert_eq!(input.transaction.nonce, Some(U256::ZERO));
        assert_eq!(input.transaction.value, None);
        assert_eq!(input.transaction.input, None);
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

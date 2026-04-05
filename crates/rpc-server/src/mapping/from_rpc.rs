use std::convert::TryFrom;

use alloy::primitives::{Bytes, U256};

use crate::{errors::ValidationError, interface as rpc};

use super::primitives::parse_u64_quantity;

impl TryFrom<rpc::EvmSimulateTransactionRequest>
    for simulation_service::SimulateEvmTransactionInput
{
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
                .unwrap_or(simulation_service::BlockRef::Latest),
            transaction: map_transaction(transaction)?,
        })
    }
}

fn map_block_ref(block: rpc::BlockRef) -> Result<simulation_service::BlockRef, ValidationError> {
    match block {
        rpc::BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(simulation_service::BlockRef::Latest),
            value => Ok(simulation_service::BlockRef::Number(parse_u64_quantity(
                value, "block",
            )?)),
        },
        rpc::BlockRef::Hash(selector) => {
            Ok(simulation_service::BlockRef::Hash(selector.block_hash))
        }
    }
}

fn map_transaction(
    transaction: rpc::Transaction,
) -> Result<simulation_service::EvmTransaction, ValidationError> {
    Ok(simulation_service::EvmTransaction {
        tx_type: infer_transaction_type(&transaction),
        requested_chain_id: transaction.chain_id,
        from: transaction.from,
        to: transaction.to,
        nonce: transaction.nonce,
        gas_limit: transaction.gas,
        value: transaction.value.unwrap_or(U256::ZERO),
        data: transaction.data.unwrap_or_else(Bytes::new),
        access_list: transaction
            .access_list
            .unwrap_or_default()
            .into_iter()
            .map(convert_access_list_item)
            .collect(),
        gas_price: transaction.gas_price,
        max_fee_per_gas: transaction.max_fee_per_gas,
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
    })
}

fn infer_transaction_type(
    transaction: &rpc::Transaction,
) -> simulation_service::EvmTransactionType {
    match transaction.tx_type {
        Some(0x0) => simulation_service::EvmTransactionType::Legacy,
        Some(0x1) => simulation_service::EvmTransactionType::AccessList,
        Some(0x2) => simulation_service::EvmTransactionType::DynamicFee,
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            simulation_service::EvmTransactionType::DynamicFee
        }
        None if transaction
            .access_list
            .as_ref()
            .is_some_and(|items| !items.is_empty()) =>
        {
            simulation_service::EvmTransactionType::AccessList
        }
        None => simulation_service::EvmTransactionType::Legacy,
        Some(_) => unreachable!("transaction type is already validated"),
    }
}

fn convert_access_list_item(item: rpc::AccessListItem) -> simulation_service::AccessListItem {
    simulation_service::AccessListItem {
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
    fn request_maps_into_service_input_with_defaults() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: Some(rpc::BlockRef::Tag("latest".to_string())),
            options: None,
            transaction: sample_transaction(),
        };

        let input: simulation_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");
        assert!(matches!(input.block, simulation_service::BlockRef::Latest));
        assert!(matches!(
            input.transaction.tx_type,
            simulation_service::EvmTransactionType::Legacy
        ));
        assert_eq!(input.transaction.requested_chain_id, None);
        assert_eq!(input.transaction.nonce, None);
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

        let input: simulation_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");

        assert!(matches!(
            input.block,
            simulation_service::BlockRef::Number(0x1234)
        ));
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

        let error = simulation_service::SimulateEvmTransactionInput::try_from(request)
            .expect_err("reserved options should be rejected");

        assert_eq!(
            error.to_string(),
            "`options.include` is reserved and not supported yet"
        );
    }

    fn sample_transaction() -> rpc::Transaction {
        rpc::Transaction {
            tx_type: None,
            chain_id: None,
            from: Address::from_str("0x1111111111111111111111111111111111111111").unwrap(),
            to: Some(Address::from_str("0x2222222222222222222222222222222222222222").unwrap()),
            nonce: None,
            gas: 0x5208,
            value: None,
            data: None,
            access_list: None,
            gas_price: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }
}

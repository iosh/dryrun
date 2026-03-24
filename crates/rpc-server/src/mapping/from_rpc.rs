use std::convert::{TryFrom, TryInto};

use crate::{errors::ValidationError, interface as rpc};

use super::primitives::{
    parse_address, parse_bytes, parse_hash, parse_u64_quantity, parse_u128_quantity,
    parse_u256_quantity,
};

impl TryFrom<rpc::EvmSimulateTransactionRequest>
    for simulation_service::SimulateEvmTransactionInput
{
    type Error = ValidationError;

    fn try_from(request: rpc::EvmSimulateTransactionRequest) -> Result<Self, Self::Error> {
        let rpc::EvmSimulateTransactionRequest {
            block,
            transaction,
            options,
        } = request;

        Ok(Self {
            block: block
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or(simulation_service::BlockRef::Latest),
            transaction: transaction.try_into()?,
            options: options
                .map(TryInto::try_into)
                .transpose()?
                .unwrap_or_default(),
        })
    }
}

impl TryFrom<rpc::BlockRef> for simulation_service::BlockRef {
    type Error = ValidationError;

    fn try_from(block: rpc::BlockRef) -> Result<Self, Self::Error> {
        block.validate()?;

        match (block.tag, block.number, block.hash) {
            (Some(_), None, None) => Ok(Self::Latest),
            (None, Some(number), None) => {
                Ok(Self::Number(parse_u64_quantity(&number, "block.number")?))
            }
            (None, None, Some(hash)) => Ok(Self::Hash(parse_hash(&hash, "block.hash")?)),
            _ => unreachable!("validated block reference must contain exactly one selector"),
        }
    }
}

impl TryFrom<rpc::Transaction> for simulation_service::EvmTransaction {
    type Error = ValidationError;

    fn try_from(transaction: rpc::Transaction) -> Result<Self, Self::Error> {
        transaction.validate()?;

        Ok(Self {
            tx_type: match transaction.tx_type.as_str() {
                "0x0" => simulation_service::EvmTransactionType::Legacy,
                "0x1" => simulation_service::EvmTransactionType::AccessList,
                "0x2" => simulation_service::EvmTransactionType::DynamicFee,
                _ => unreachable!("validated transaction type must be supported"),
            },
            chain_id: parse_u64_quantity(&transaction.chain_id, "transaction.chainId")?,
            from: parse_address(&transaction.from, "transaction.from")?,
            to: transaction
                .to
                .as_deref()
                .map(|value| parse_address(value, "transaction.to"))
                .transpose()?,
            nonce: parse_u64_quantity(&transaction.nonce, "transaction.nonce")?,
            gas_limit: parse_u64_quantity(&transaction.gas, "transaction.gas")?,
            value: parse_u256_quantity(&transaction.value, "transaction.value")?,
            data: parse_bytes(&transaction.data, "transaction.data")?,
            access_list: transaction
                .access_list
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(index, item)| convert_access_list_item(item, index))
                .collect::<Result<Vec<_>, _>>()?,
            gas_price: transaction
                .gas_price
                .as_deref()
                .map(|value| parse_u128_quantity(value, "transaction.gasPrice"))
                .transpose()?,
            max_fee_per_gas: transaction
                .max_fee_per_gas
                .as_deref()
                .map(|value| parse_u128_quantity(value, "transaction.maxFeePerGas"))
                .transpose()?,
            max_priority_fee_per_gas: transaction
                .max_priority_fee_per_gas
                .as_deref()
                .map(|value| parse_u128_quantity(value, "transaction.maxPriorityFeePerGas"))
                .transpose()?,
        })
    }
}

impl TryFrom<rpc::SimulationOptions> for simulation_service::SimulationOptions {
    type Error = ValidationError;

    fn try_from(options: rpc::SimulationOptions) -> Result<Self, Self::Error> {
        options.validate()?;

        Ok(Self {
            include_logs: options.include_logs.unwrap_or(true),
            include_asset_changes: options.include_asset_changes.unwrap_or(true),
        })
    }
}

fn convert_access_list_item(
    item: rpc::AccessListItem,
    index: usize,
) -> Result<simulation_service::AccessListItem, ValidationError> {
    Ok(simulation_service::AccessListItem {
        address: parse_address(
            &item.address,
            &format!("transaction.accessList[{index}].address"),
        )?,
        storage_keys: item
            .storage_keys
            .into_iter()
            .enumerate()
            .map(|(slot_index, slot)| {
                parse_hash(
                    &slot,
                    &format!("transaction.accessList[{index}].storageKeys[{slot_index}]"),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use crate::interface as rpc;

    #[test]
    fn request_maps_into_service_input() {
        let request = rpc::EvmSimulateTransactionRequest {
            block: Some(rpc::BlockRef {
                tag: Some("latest".to_string()),
                number: None,
                hash: None,
            }),
            transaction: rpc::Transaction {
                tx_type: "0x2".to_string(),
                chain_id: "0x1".to_string(),
                from: "0x1111111111111111111111111111111111111111".to_string(),
                to: Some("0x2222222222222222222222222222222222222222".to_string()),
                nonce: "0x0".to_string(),
                gas: "0x5208".to_string(),
                value: "0x0".to_string(),
                data: "0x1234".to_string(),
                access_list: None,
                gas_price: None,
                max_fee_per_gas: Some("0x3b9aca00".to_string()),
                max_priority_fee_per_gas: Some("0x1".to_string()),
                blob_versioned_hashes: None,
                max_fee_per_blob_gas: None,
                sidecar: None,
                authorization_list: None,
            },
            options: Some(rpc::SimulationOptions {
                include_logs: Some(false),
                include_asset_changes: Some(true),
                include_trace: Some(false),
                include_state_changes: Some(false),
            }),
        };

        let input: simulation_service::SimulateEvmTransactionInput =
            request.try_into().expect("request should map");
        assert!(matches!(input.block, simulation_service::BlockRef::Latest));
        assert_eq!(input.transaction.chain_id, 1);
        assert_eq!(input.transaction.gas_limit, 0x5208);
        assert!(!input.options.include_logs);
    }
}

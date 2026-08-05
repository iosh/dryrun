use std::convert::TryFrom;

use simulation_transaction::{TransactionType, TransactionVariantRequest};

use crate::{errors::ValidationError, interface as rpc};

use super::shared::parse_u64_param;

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
                .unwrap_or(evm_service::EvmBlockSelector::Latest),
            transaction: map_transaction(transaction)?,
        })
    }
}

fn map_block_ref(block: rpc::BlockRef) -> Result<evm_service::EvmBlockSelector, ValidationError> {
    match block {
        rpc::BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(evm_service::EvmBlockSelector::Latest),
            "safe" => Ok(evm_service::EvmBlockSelector::Safe),
            "finalized" => Ok(evm_service::EvmBlockSelector::Finalized),
            value => Ok(evm_service::EvmBlockSelector::Number(parse_u64_param(
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
) -> Result<evm_service::EvmTransactionRequest, ValidationError> {
    let transaction_type = map_transaction_type(transaction.tx_type)?;
    let transaction_type = TransactionType::infer(
        transaction_type,
        transaction.access_list.is_some(),
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some(),
    );
    let variant = TransactionVariantRequest::try_new(
        transaction_type,
        transaction
            .access_list
            .map(|items| items.into_iter().map(to_service_access_list_item).collect()),
        transaction.gas_price,
        transaction.max_fee_per_gas,
        transaction.max_priority_fee_per_gas,
    )
    .map_err(|error| ValidationError::invalid_params(error.to_string()))?;

    Ok(evm_service::EvmTransactionRequest {
        from: transaction.from,
        to: transaction.to,
        nonce: transaction.nonce,
        gas_limit: transaction.gas,
        value: transaction.value,
        data: transaction.data,
        chain_id: transaction.chain_id,
        variant,
    })
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

fn to_service_access_list_item(item: rpc::AccessListItem) -> evm_service::AccessListItem {
    evm_service::AccessListItem {
        address: item.address,
        storage_keys: item.storage_keys,
    }
}

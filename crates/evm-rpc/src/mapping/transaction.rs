use evm_simulation::{
    AccessListItem, Authorization, PartialTransaction, SignedAuthorization, TransactionInput,
    TxType,
};

use crate::{errors::ValidationError, interface as rpc};

pub(super) fn map_transaction(
    transaction: rpc::Transaction,
) -> Result<TransactionInput, ValidationError> {
    let rpc::Transaction {
        tx_type,
        chain_id,
        from,
        to,
        nonce,
        gas,
        value,
        data,
        access_list,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        max_fee_per_blob_gas,
        blob_versioned_hashes,
        authorization_list,
    } = transaction;
    let transaction_type = tx_type
        .map(|value| {
            TxType::try_from(value).map_err(|_| {
                ValidationError::invalid_params(
                    "`transaction.type` must be one of `0x0`, `0x1`, `0x2`, `0x3`, or `0x4`",
                )
            })
        })
        .transpose()?;

    Ok(TransactionInput::Partial(PartialTransaction {
        from,
        to,
        nonce,
        gas_limit: gas,
        value,
        input: data,
        chain_id,
        transaction_type,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        max_fee_per_blob_gas,
        access_list: map_access_list(access_list),
        blob_versioned_hashes,
        authorization_list: authorization_list
            .map(|items| items.into_iter().map(map_signed_authorization).collect()),
    }))
}

fn map_access_list(items: Option<Vec<rpc::AccessListItem>>) -> Option<Vec<AccessListItem>> {
    items.map(|items| items.into_iter().map(map_access_list_item).collect())
}

fn map_access_list_item(item: rpc::AccessListItem) -> AccessListItem {
    AccessListItem {
        address: item.address,
        storage_keys: item.storage_keys,
    }
}

fn map_signed_authorization(authorization: rpc::SignedAuthorization) -> SignedAuthorization {
    SignedAuthorization::new_unchecked(
        Authorization {
            chain_id: authorization.chain_id,
            address: authorization.address,
            nonce: authorization.nonce,
        },
        authorization.y_parity,
        authorization.r,
        authorization.s,
    )
}

use evm_simulation::{
    AccessListItem, Authorization, PartialTransaction, PartialTransactionVariant,
    SignedAuthorization, TransactionInput,
};

use crate::{errors::ValidationError, interface as rpc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RpcTransactionType {
    Legacy,
    Eip2930,
    Eip1559,
    Eip4844,
    Eip7702,
}

impl RpcTransactionType {
    fn classify(transaction: &rpc::Transaction) -> Result<Self, ValidationError> {
        if let Some(transaction_type) = transaction.tx_type {
            return match transaction_type {
                0x0 => Ok(Self::Legacy),
                0x1 => Ok(Self::Eip2930),
                0x2 => Ok(Self::Eip1559),
                0x3 => Ok(Self::Eip4844),
                0x4 => Ok(Self::Eip7702),
                _ => Err(ValidationError::invalid_params(
                    "`transaction.type` must be one of `0x0`, `0x1`, `0x2`, `0x3`, or `0x4`",
                )),
            };
        }

        Ok(if transaction.authorization_list.is_some() {
            Self::Eip7702
        } else if transaction.max_fee_per_blob_gas.is_some()
            || transaction.blob_versioned_hashes.is_some()
        {
            Self::Eip4844
        } else if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some()
        {
            Self::Eip1559
        } else if transaction.access_list.is_some() {
            Self::Eip2930
        } else {
            Self::Legacy
        })
    }
}

pub(super) fn map_transaction(
    transaction: rpc::Transaction,
) -> Result<TransactionInput, ValidationError> {
    let transaction_type = RpcTransactionType::classify(&transaction)?;
    let rpc::Transaction {
        tx_type: _,
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
    let variant = match transaction_type {
        RpcTransactionType::Legacy => PartialTransactionVariant::Legacy { gas_price },
        RpcTransactionType::Eip2930 => PartialTransactionVariant::Eip2930 {
            gas_price,
            access_list: map_access_list(access_list),
        },
        RpcTransactionType::Eip1559 => PartialTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list: map_access_list(access_list),
        },
        RpcTransactionType::Eip4844 => PartialTransactionVariant::Eip4844 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            max_fee_per_blob_gas,
            access_list: map_access_list(access_list),
            blob_versioned_hashes: blob_versioned_hashes.unwrap_or_default(),
        },
        RpcTransactionType::Eip7702 => PartialTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list: map_access_list(access_list),
            authorization_list: authorization_list
                .unwrap_or_default()
                .into_iter()
                .map(map_signed_authorization)
                .collect(),
        },
    };

    Ok(TransactionInput::Partial(PartialTransaction {
        from,
        to,
        nonce,
        gas_limit: gas,
        value,
        input: data,
        chain_id: Some(chain_id),
        variant,
    }))
}

fn map_access_list(items: Option<Vec<rpc::AccessListItem>>) -> Vec<AccessListItem> {
    items
        .unwrap_or_default()
        .into_iter()
        .map(map_access_list_item)
        .collect()
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

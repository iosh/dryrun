use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_service::{AccessListItem, core_space as service_core_space};
use serde::Deserialize;
use simulation_transaction::{TransactionType, TransactionVariantRequest};

use super::chain_id_from_wire;
use crate::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SimulateCoreSpaceTransactionRequest {
    transaction: CoreSpaceTransactionRequest,
    #[serde(default)]
    epoch: Option<EpochNumber>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoreSpaceTransactionRequest {
    from: Option<RpcAddress>,
    to: Option<RpcAddress>,
    gas_price: Option<U256>,
    gas: Option<U256>,
    value: Option<U256>,
    data: Option<CoreSpaceRpcBytes>,
    nonce: Option<U256>,
    storage_limit: Option<U64>,
    access_list: Option<Vec<CoreSpaceAccessListItem>>,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
    #[serde(rename = "type")]
    transaction_type: Option<U64>,
    chain_id: Option<U256>,
    epoch_height: Option<U256>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoreSpaceAccessListItem {
    address: RpcAddress,
    storage_keys: Vec<H256>,
}

impl SimulateCoreSpaceTransactionRequest {
    pub(crate) fn try_into_service_input(
        self,
        expected_network: Network,
    ) -> Result<service_core_space::SimulateCoreSpaceTransactionInput, ValidationError> {
        Ok(service_core_space::SimulateCoreSpaceTransactionInput {
            epoch: map_core_space_epoch(self.epoch)?,
            transaction: map_core_space_transaction(self.transaction, expected_network)?,
        })
    }
}

fn map_core_space_epoch(
    epoch: Option<EpochNumber>,
) -> Result<service_core_space::CoreSpaceEpochRef, ValidationError> {
    match epoch.unwrap_or(EpochNumber::LatestState) {
        EpochNumber::LatestState => Ok(service_core_space::CoreSpaceEpochRef::LatestState),
        EpochNumber::Num(number) => Ok(service_core_space::CoreSpaceEpochRef::Number(
            number.as_u64(),
        )),
        _ => Err(ValidationError::not_supported(
            "`epoch` only supports `latest_state` or a hex epoch number",
        )),
    }
}

fn map_core_space_transaction(
    transaction: CoreSpaceTransactionRequest,
    expected_network: Network,
) -> Result<service_core_space::CoreSpaceTransactionRequest, ValidationError> {
    validate_core_space_address_networks(&transaction, expected_network)?;

    let transaction_type = map_transaction_type(transaction.transaction_type)?;
    let transaction_type = TransactionType::infer(
        transaction_type,
        transaction.access_list.is_some(),
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some(),
    );
    validate_legacy_access_list(&transaction, transaction_type)?;

    let CoreSpaceTransactionRequest {
        from,
        to,
        gas_price,
        gas,
        value,
        data,
        nonce,
        storage_limit,
        access_list,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        chain_id,
        epoch_height,
        ..
    } = transaction;

    let from = require_core_space_field(from, "transaction.from")?;
    let chain_id = require_core_space_field(chain_id, "transaction.chainId")?;
    let variant = TransactionVariantRequest::try_new(
        transaction_type,
        access_list.map(map_core_space_access_list),
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
    )
    .map_err(|error| ValidationError::invalid_params(error.to_string()))?;

    Ok(service_core_space::CoreSpaceTransactionRequest {
        transaction: conflux_service::ConfluxTransactionRequest {
            from: from.hex_address,
            to: to.map(|address| address.hex_address),
            nonce,
            gas_limit: gas,
            value,
            input: data.map(|data| data.into_vec()),
            chain_id: chain_id_from_wire(chain_id)?,
            variant,
        },
        storage_limit: storage_limit.map(|value| value.as_u64()),
        epoch_height: epoch_height.map(epoch_height_from_wire).transpose()?,
    })
}

fn epoch_height_from_wire(epoch_height: U256) -> Result<u64, ValidationError> {
    u64::try_from(epoch_height).map_err(|_| {
        ValidationError::invalid_params(
            "`transaction.epochHeight` must fit into an unsigned 64-bit integer",
        )
    })
}

fn map_transaction_type(
    transaction_type: Option<U64>,
) -> Result<Option<TransactionType>, ValidationError> {
    match transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => Ok(Some(TransactionType::Legacy)),
        Some(0x1) => Ok(Some(TransactionType::AccessList)),
        Some(0x2) => Ok(Some(TransactionType::DynamicFee)),
        Some(_) => Err(ValidationError::invalid_params(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        )),
        None => Ok(None),
    }
}

fn validate_legacy_access_list(
    transaction: &CoreSpaceTransactionRequest,
    transaction_type: TransactionType,
) -> Result<(), ValidationError> {
    if transaction_type == TransactionType::Legacy && transaction.access_list.is_some() {
        return Err(ValidationError::invalid_params(
            "CIP-155 transactions cannot include `transaction.accessList`",
        ));
    }

    Ok(())
}

fn validate_core_space_address_networks(
    transaction: &CoreSpaceTransactionRequest,
    expected_network: Network,
) -> Result<(), ValidationError> {
    if let Some(from) = transaction.from.as_ref() {
        validate_core_space_address_network(from, expected_network, "transaction.from")?;
    }

    if let Some(to) = transaction.to.as_ref() {
        validate_core_space_address_network(to, expected_network, "transaction.to")?;
    }

    if let Some(access_list) = transaction.access_list.as_ref() {
        for (index, item) in access_list.iter().enumerate() {
            validate_core_space_address_network(
                &item.address,
                expected_network,
                &format!("transaction.accessList[{index}].address"),
            )?;
        }
    }

    Ok(())
}

fn validate_core_space_address_network(
    address: &RpcAddress,
    expected_network: Network,
    field: &str,
) -> Result<(), ValidationError> {
    if address.network != expected_network {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` uses address network {}, expected {}",
            address.network, expected_network
        )));
    }

    Ok(())
}

fn require_core_space_field<T>(value: Option<T>, field: &str) -> Result<T, ValidationError> {
    value.ok_or_else(|| ValidationError::invalid_params(format!("`{field}` is required")))
}

fn map_core_space_access_list(items: Vec<CoreSpaceAccessListItem>) -> Vec<AccessListItem> {
    items
        .into_iter()
        .map(|item| AccessListItem {
            address: item.address.hex_address,
            storage_keys: item.storage_keys,
        })
        .collect()
}

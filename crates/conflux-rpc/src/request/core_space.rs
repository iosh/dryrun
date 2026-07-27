use alloy_primitives::{Bytes as AlloyBytes, U256 as AlloyU256};
use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_service::core_space as service_core_space;
use serde::Deserialize;
use simulation_transaction::{
    AccessListItem as SimulationAccessListItem, TransactionRequest as SimulationTransactionRequest,
    TransactionType,
};

use super::primitives::{to_alloy_address, to_alloy_b256, to_alloy_u256};
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

    let transaction_type = to_transaction_type(transaction.transaction_type)?;
    let tx_type = resolve_transaction_type(&transaction, transaction_type);
    validate_core_space_transaction_shape(&transaction, tx_type)?;

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
    let request = SimulationTransactionRequest {
        from: to_alloy_address(from.hex_address),
        to: to.map(|address| to_alloy_address(address.hex_address)),
        nonce: nonce.map(to_alloy_u256),
        gas_limit: gas.map(to_alloy_u256),
        value: value.map(to_alloy_u256),
        input: data.map(|data| AlloyBytes::from(data.into_vec())),
        chain_id: chain_id.map(to_alloy_u256),
        transaction_type,
        access_list: access_list.map(map_core_space_access_list),
        gas_price: gas_price.map(to_alloy_u256),
        max_fee_per_gas: max_fee_per_gas.map(to_alloy_u256),
        max_priority_fee_per_gas: max_priority_fee_per_gas.map(to_alloy_u256),
    };

    Ok(service_core_space::CoreSpaceTransactionRequest {
        transaction: request,
        storage_limit: storage_limit.map(|value| AlloyU256::from(value.as_u64())),
        epoch_height: epoch_height.map(to_alloy_u256),
    })
}

fn resolve_transaction_type(
    transaction: &CoreSpaceTransactionRequest,
    transaction_type: Option<TransactionType>,
) -> TransactionType {
    match transaction_type {
        Some(transaction_type) => transaction_type,
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            TransactionType::DynamicFee
        }
        None if transaction.access_list.is_some() => TransactionType::AccessList,
        None => TransactionType::Legacy,
    }
}

fn to_transaction_type(
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

fn validate_core_space_transaction_shape(
    transaction: &CoreSpaceTransactionRequest,
    tx_type: TransactionType,
) -> Result<(), ValidationError> {
    let has_dynamic_fee =
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some();

    match tx_type {
        TransactionType::Legacy => {
            if transaction.access_list.is_some() {
                return Err(ValidationError::invalid_params(
                    "CIP-155 transactions cannot include `transaction.accessList`",
                ));
            }

            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "CIP-155 transactions cannot include CIP-1559 fee fields",
                ));
            }
        }
        TransactionType::AccessList => {
            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "CIP-2930 transactions cannot include CIP-1559 fee fields",
                ));
            }
        }
        TransactionType::DynamicFee => {
            if transaction.gas_price.is_some() {
                return Err(ValidationError::invalid_params(
                    "CIP-1559 transactions cannot include `transaction.gasPrice`",
                ));
            }
        }
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

fn map_core_space_access_list(
    items: Vec<CoreSpaceAccessListItem>,
) -> Vec<SimulationAccessListItem> {
    items
        .into_iter()
        .map(|item| SimulationAccessListItem {
            address: to_alloy_address(item.address.hex_address),
            storage_keys: item.storage_keys.into_iter().map(to_alloy_b256).collect(),
        })
        .collect()
}

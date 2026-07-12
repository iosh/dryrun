use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_primitives::Bytes as NativeRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_service::native as service_native;
use serde::Deserialize;

use super::shared::{u256_to_u32_quantity, u256_to_u64_quantity};
use crate::rpc::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::rpc) struct SimulateNativeTransactionRequest {
    transaction: NativeTransactionRequest,
    #[serde(default)]
    epoch: Option<EpochNumber>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeTransactionRequest {
    from: Option<RpcAddress>,
    to: Option<RpcAddress>,
    gas_price: Option<U256>,
    gas: Option<U256>,
    value: Option<U256>,
    data: Option<NativeRpcBytes>,
    nonce: Option<U256>,
    storage_limit: Option<U64>,
    access_list: Option<Vec<NativeAccessListItem>>,
    max_fee_per_gas: Option<U256>,
    max_priority_fee_per_gas: Option<U256>,
    #[serde(rename = "type")]
    transaction_type: Option<U64>,
    chain_id: Option<U256>,
    epoch_height: Option<U256>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeAccessListItem {
    address: RpcAddress,
    storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeTransactionType {
    Cip155,
    Cip2930,
    Cip1559,
}

impl SimulateNativeTransactionRequest {
    pub(in crate::rpc) fn try_into_service_input(
        self,
        expected_network: Network,
    ) -> Result<service_native::SimulateNativeTransactionInput, ValidationError> {
        Ok(service_native::SimulateNativeTransactionInput {
            epoch: map_native_epoch(self.epoch)?,
            transaction: map_native_transaction(self.transaction, expected_network)?,
        })
    }
}

fn map_native_epoch(
    epoch: Option<EpochNumber>,
) -> Result<service_native::NativeEpochRef, ValidationError> {
    match epoch.unwrap_or(EpochNumber::LatestState) {
        EpochNumber::LatestState => Ok(service_native::NativeEpochRef::LatestState),
        EpochNumber::Num(number) => {
            Ok(service_native::NativeEpochRef::Number(number.as_u64()))
        }
        _ => Err(ValidationError::not_supported(
            "`epoch` only supports `latest_state` or a hex epoch number",
        )),
    }
}

fn map_native_transaction(
    transaction: NativeTransactionRequest,
    expected_network: Network,
) -> Result<service_native::NativeTransaction, ValidationError> {
    validate_native_address_networks(&transaction, expected_network)?;

    let tx_type = infer_native_transaction_type(&transaction)?;
    validate_native_transaction_shape(&transaction, tx_type)?;

    let NativeTransactionRequest {
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

    let from = require_native_field(from, "transaction.from")?;
    let nonce = require_native_field(nonce, "transaction.nonce")?;
    let gas_limit = require_native_field(gas, "transaction.gas")?;
    let storage_limit = require_native_field(storage_limit, "transaction.storageLimit")?.as_u64();
    let epoch_height = u256_to_u64_quantity(
        require_native_field(epoch_height, "transaction.epochHeight")?,
        "transaction.epochHeight",
    )?;
    let chain_id = u256_to_u32_quantity(
        require_native_field(chain_id, "transaction.chainId")?,
        "transaction.chainId",
    )?;

    let variant = match tx_type {
        NativeTransactionType::Cip155 => service_native::NativeTransactionVariant::Cip155 {
            gas_price: require_native_field(gas_price, "transaction.gasPrice")?,
        },
        NativeTransactionType::Cip2930 => service_native::NativeTransactionVariant::Cip2930 {
            gas_price: require_native_field(gas_price, "transaction.gasPrice")?,
            access_list: map_native_access_list(access_list.unwrap_or_default()),
        },
        NativeTransactionType::Cip1559 => service_native::NativeTransactionVariant::Cip1559 {
            max_fee_per_gas: require_native_field(max_fee_per_gas, "transaction.maxFeePerGas")?,
            max_priority_fee_per_gas: require_native_field(
                max_priority_fee_per_gas,
                "transaction.maxPriorityFeePerGas",
            )?,
            access_list: map_native_access_list(access_list.unwrap_or_default()),
        },
    };

    Ok(service_native::NativeTransaction {
        from: from.hex_address,
        to: to.map(|address| address.hex_address),
        nonce,
        gas_limit,
        value: value.unwrap_or_default(),
        data: data.unwrap_or_default().into_vec().into(),
        storage_limit,
        epoch_height,
        chain_id,
        variant,
    })
}

fn infer_native_transaction_type(
    transaction: &NativeTransactionRequest,
) -> Result<NativeTransactionType, ValidationError> {
    match transaction.transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => Ok(NativeTransactionType::Cip155),
        Some(0x1) => Ok(NativeTransactionType::Cip2930),
        Some(0x2) => Ok(NativeTransactionType::Cip1559),
        Some(_) => Err(ValidationError::invalid_params(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        )),
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            Ok(NativeTransactionType::Cip1559)
        }
        None if transaction.access_list.is_some() => Ok(NativeTransactionType::Cip2930),
        None => Ok(NativeTransactionType::Cip155),
    }
}

fn validate_native_transaction_shape(
    transaction: &NativeTransactionRequest,
    tx_type: NativeTransactionType,
) -> Result<(), ValidationError> {
    let has_dynamic_fee =
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some();

    match tx_type {
        NativeTransactionType::Cip155 => {
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
        NativeTransactionType::Cip2930 => {
            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "CIP-2930 transactions cannot include CIP-1559 fee fields",
                ));
            }
        }
        NativeTransactionType::Cip1559 => {
            if transaction.gas_price.is_some() {
                return Err(ValidationError::invalid_params(
                    "CIP-1559 transactions cannot include `transaction.gasPrice`",
                ));
            }
        }
    }

    Ok(())
}

fn validate_native_address_networks(
    transaction: &NativeTransactionRequest,
    expected_network: Network,
) -> Result<(), ValidationError> {
    if let Some(from) = transaction.from.as_ref() {
        validate_native_address_network(from, expected_network, "transaction.from")?;
    }

    if let Some(to) = transaction.to.as_ref() {
        validate_native_address_network(to, expected_network, "transaction.to")?;
    }

    if let Some(access_list) = transaction.access_list.as_ref() {
        for (index, item) in access_list.iter().enumerate() {
            validate_native_address_network(
                &item.address,
                expected_network,
                &format!("transaction.accessList[{index}].address"),
            )?;
        }
    }

    Ok(())
}

fn validate_native_address_network(
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

fn require_native_field<T>(value: Option<T>, field: &str) -> Result<T, ValidationError> {
    value.ok_or_else(|| ValidationError::invalid_params(format!("`{field}` is required")))
}

fn map_native_access_list(items: Vec<NativeAccessListItem>) -> Vec<service_native::AccessListItem> {
    items
        .into_iter()
        .map(|item| service_native::AccessListItem {
            address: item.address.hex_address,
            storage_keys: item.storage_keys,
        })
        .collect()
}

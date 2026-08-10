use alloy_primitives::Bytes;
use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_provider::Network as ProviderNetwork;
use conflux_service::core_space as service_core_space;
use serde::Deserialize;

use super::{cfx_h256_to_alloy, cfx_u256_to_alloy, u64_param};
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
    ) -> Result<service_core_space::CoreSpaceSimulationInput, ValidationError> {
        Ok(service_core_space::CoreSpaceSimulationInput {
            epoch: map_core_space_epoch(self.epoch)?,
            transaction: map_core_space_transaction(self.transaction, expected_network)?,
        })
    }
}

fn map_core_space_epoch(
    epoch: Option<EpochNumber>,
) -> Result<service_core_space::CoreSpaceBlockSelector, ValidationError> {
    match epoch.unwrap_or(EpochNumber::LatestState) {
        EpochNumber::LatestState => Ok(service_core_space::CoreSpaceBlockSelector::LatestState),
        EpochNumber::Num(number) => Ok(service_core_space::CoreSpaceBlockSelector::Number(
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
) -> Result<service_core_space::CoreSpaceTransactionInput, ValidationError> {
    let transaction_type = infer_transaction_type(&transaction)?;
    validate_core_space_address_networks(&transaction, transaction_type, expected_network)?;

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
    let variant = match transaction_type {
        CoreSpaceTransactionType::Cip155 => {
            service_core_space::CoreSpacePartialTransactionVariant::Cip155 {
                gas_price: gas_price.map(cfx_u256_to_alloy),
            }
        }
        CoreSpaceTransactionType::Cip2930 => {
            service_core_space::CoreSpacePartialTransactionVariant::Cip2930 {
                gas_price: gas_price.map(cfx_u256_to_alloy),
                access_list: access_list
                    .map(map_core_space_access_list)
                    .transpose()?
                    .unwrap_or_default(),
            }
        }
        CoreSpaceTransactionType::Cip1559 => {
            service_core_space::CoreSpacePartialTransactionVariant::Cip1559 {
                max_fee_per_gas: max_fee_per_gas.map(cfx_u256_to_alloy),
                max_priority_fee_per_gas: max_priority_fee_per_gas.map(cfx_u256_to_alloy),
                access_list: access_list
                    .map(map_core_space_access_list)
                    .transpose()?
                    .unwrap_or_default(),
            }
        }
    };

    Ok(service_core_space::CoreSpaceTransactionInput::Partial(
        service_core_space::CoreSpacePartialTransaction {
            from: map_core_space_address(from)?,
            to: to.map(map_core_space_address).transpose()?,
            nonce: nonce.map(cfx_u256_to_alloy),
            gas_limit: gas.map(cfx_u256_to_alloy),
            value: value.map(cfx_u256_to_alloy),
            data: data.map(|data| Bytes::from(data.into_vec())),
            chain_id: Some(u32_param(chain_id, "transaction.chainId")?),
            variant,
            storage_limit: storage_limit.map(|value| value.as_u64()),
            epoch_height: epoch_height
                .map(|value| u64_param(value, "transaction.epochHeight"))
                .transpose()?,
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreSpaceTransactionType {
    Cip155,
    Cip2930,
    Cip1559,
}

fn infer_transaction_type(
    transaction: &CoreSpaceTransactionRequest,
) -> Result<CoreSpaceTransactionType, ValidationError> {
    let explicit = match transaction.transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => Some(CoreSpaceTransactionType::Cip155),
        Some(0x1) => Some(CoreSpaceTransactionType::Cip2930),
        Some(0x2) => Some(CoreSpaceTransactionType::Cip1559),
        Some(_) => Err(ValidationError::invalid_params(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        ))?,
        None => None,
    };
    let inferred = if transaction.max_fee_per_gas.is_some()
        || transaction.max_priority_fee_per_gas.is_some()
    {
        CoreSpaceTransactionType::Cip1559
    } else if transaction.access_list.is_some() {
        CoreSpaceTransactionType::Cip2930
    } else {
        CoreSpaceTransactionType::Cip155
    };
    Ok(explicit.unwrap_or(inferred))
}

fn u32_param(value: U256, field: &str) -> Result<u32, ValidationError> {
    u32::try_from(value).map_err(|_| {
        ValidationError::invalid_params(format!(
            "`{field}` value {value:#x} exceeds the simulator maximum {:#x}",
            u32::MAX
        ))
    })
}

fn validate_core_space_address_networks(
    transaction: &CoreSpaceTransactionRequest,
    transaction_type: CoreSpaceTransactionType,
    expected_network: Network,
) -> Result<(), ValidationError> {
    if let Some(from) = transaction.from.as_ref() {
        validate_core_space_address_network(from, expected_network, "transaction.from")?;
    }

    if let Some(to) = transaction.to.as_ref() {
        validate_core_space_address_network(to, expected_network, "transaction.to")?;
    }

    if transaction_type != CoreSpaceTransactionType::Cip155
        && let Some(access_list) = transaction.access_list.as_ref()
    {
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
) -> Result<Vec<service_core_space::CoreSpaceAccessListItem>, ValidationError> {
    items
        .into_iter()
        .map(|item| {
            Ok(service_core_space::CoreSpaceAccessListItem {
                address: map_core_space_address(item.address)?,
                storage_keys: item
                    .storage_keys
                    .into_iter()
                    .map(cfx_h256_to_alloy)
                    .collect(),
            })
        })
        .collect()
}

fn map_core_space_address(
    address: RpcAddress,
) -> Result<service_core_space::CoreAddress, ValidationError> {
    let mut bytes = [0_u8; 20];
    bytes.copy_from_slice(address.hex_address.as_bytes());
    let network = match address.network {
        Network::Main => ProviderNetwork::Main,
        Network::Test => ProviderNetwork::Test,
        Network::Id(id) => ProviderNetwork::Id(id),
    };

    service_core_space::CoreAddress::from_bytes(bytes, network)
        .map_err(|error| ValidationError::invalid_params(error.to_string()))
}

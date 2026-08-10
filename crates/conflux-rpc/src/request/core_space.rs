use alloy_primitives::Bytes;
use cfx_addr::Network;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_primitives::Bytes as CoreSpaceRpcBytes;
use cfx_types::{H256, U64, U256};
use conflux_provider::Network as ProviderNetwork;
use conflux_service::core_space as service_core_space;
use serde::Deserialize;
use simulation_transaction::TransactionType;

use super::{cfx_h256_to_alloy, cfx_u256_to_alloy, u64_param, u128_param};
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
    let access_list = access_list.map(map_core_space_access_list).transpose()?;
    let variant = service_core_space::CoreSpaceTransactionVariantRequest::try_new(
        transaction_type,
        access_list,
        gas_price
            .map(|value| u128_param(value, "transaction.gasPrice"))
            .transpose()?,
        max_fee_per_gas
            .map(|value| u128_param(value, "transaction.maxFeePerGas"))
            .transpose()?,
        max_priority_fee_per_gas
            .map(|value| u128_param(value, "transaction.maxPriorityFeePerGas"))
            .transpose()?,
    )
    .map_err(|error| ValidationError::invalid_params(error.to_string()))?;

    Ok(service_core_space::CoreSpaceTransactionInput {
        transaction: service_core_space::CoreSpaceTransactionRequest {
            from: map_core_space_address(from)?,
            to: to.map(map_core_space_address).transpose()?,
            nonce: nonce
                .map(|value| u64_param(value, "transaction.nonce"))
                .transpose()?,
            gas_limit: gas
                .map(|value| u64_param(value, "transaction.gas"))
                .transpose()?,
            value: value.map(cfx_u256_to_alloy),
            data: data.map(|data| Bytes::from(data.into_vec())),
            chain_id: u64_param(chain_id, "transaction.chainId")?,
            variant,
        },
        storage_limit: storage_limit.map(|value| value.as_u64()),
        epoch_height: epoch_height
            .map(|value| u64_param(value, "transaction.epochHeight"))
            .transpose()?,
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

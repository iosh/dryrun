use std::str::FromStr;

use cfx_addr::Network;
use cfx_bytes::Bytes;
use cfx_rpc_cfx_types::{EpochNumber, RpcAddress};
use cfx_rpc_eth_types::TransactionRequest;
use cfx_rpc_primitives::Bytes as NativeRpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::{espace as service_espace, native as service_native};
use serde::Deserialize;
use serde_json::Value;

use super::error::ValidationError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SimulateEspaceTransactionRequest {
    transaction: TransactionRequest,
    #[serde(default)]
    block: Option<BlockRef>,
    #[serde(default)]
    options: Option<SimulateTransactionOptions>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SimulateNativeTransactionRequest {
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
enum EspaceTransactionType {
    Legacy,
    Eip2930,
    Eip1559,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeTransactionType {
    Cip155,
    Cip2930,
    Cip1559,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum BlockRef {
    Tag(String),
    Hash(BlockHashRef),
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlockHashRef {
    block_hash: H256,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SimulateTransactionOptions {
    #[serde(default)]
    state_overrides: Option<Value>,
    #[serde(default)]
    block_overrides: Option<Value>,
    #[serde(default)]
    include: Option<Value>,
}

impl TryFrom<SimulateEspaceTransactionRequest> for service_espace::SimulateEspaceTransactionInput {
    type Error = ValidationError;

    fn try_from(request: SimulateEspaceTransactionRequest) -> Result<Self, Self::Error> {
        request.validate()?;

        Ok(Self {
            block: request
                .block
                .map(map_block_ref)
                .transpose()?
                .unwrap_or(service_espace::EspaceBlockRef::Latest),
            transaction: map_transaction(request.transaction)?,
        })
    }
}

impl SimulateNativeTransactionRequest {
    pub(super) fn try_into_service_input(
        self,
        expected_network: Network,
    ) -> Result<service_native::SimulateNativeTransactionInput, ValidationError> {
        Ok(service_native::SimulateNativeTransactionInput {
            epoch: map_native_epoch(self.epoch)?,
            transaction: map_native_transaction(self.transaction, expected_network)?,
        })
    }
}

impl SimulateEspaceTransactionRequest {
    fn validate(&self) -> Result<(), ValidationError> {
        validate_transaction(&self.transaction)?;

        if let Some(block) = &self.block {
            block.validate()?;
        }

        if let Some(options) = &self.options {
            options.validate()?;
        }

        Ok(())
    }
}

impl BlockRef {
    fn validate(&self) -> Result<(), ValidationError> {
        match self {
            Self::Tag(value) => match value.as_str() {
                "latest" => Ok(()),
                "pending" | "safe" | "finalized" => Err(ValidationError::not_supported(
                    "`block` only supports `latest` or a hex block number",
                )),
                value if H256::from_str(value).is_ok() => Err(ValidationError::not_supported(
                    "`block` does not support block hash selectors yet",
                )),
                value => parse_u64_quantity(value, "block").map(|_| ()),
            },
            Self::Hash(_) => Err(ValidationError::not_supported(
                "`block.blockHash` is not supported yet",
            )),
        }
    }
}

impl SimulateTransactionOptions {
    fn validate(&self) -> Result<(), ValidationError> {
        validate_reserved_option("stateOverrides", self.state_overrides.as_ref())?;
        validate_reserved_option("blockOverrides", self.block_overrides.as_ref())?;
        validate_reserved_option("include", self.include.as_ref())?;

        Ok(())
    }
}

fn validate_transaction(transaction: &TransactionRequest) -> Result<(), ValidationError> {
    if transaction.authorization_list.is_some() {
        return Err(ValidationError::not_supported(
            "`transaction.authorizationList` is not supported yet",
        ));
    }

    Ok(())
}
fn require_transaction_from(transaction: &TransactionRequest) -> Result<Address, ValidationError> {
    transaction
        .from
        .ok_or_else(|| ValidationError::invalid_params("`transaction.from` is required"))
}

fn require_transaction_gas(transaction: &TransactionRequest) -> Result<U256, ValidationError> {
    transaction
        .gas
        .ok_or_else(|| ValidationError::invalid_params("`transaction.gas` is required"))
}

fn u256_to_u64_quantity(value: U256, field: &str) -> Result<u64, ValidationError> {
    if value > U256::from(u64::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        )));
    }

    Ok(value.as_u64())
}

fn u256_to_u32_quantity(value: U256, field: &str) -> Result<u32, ValidationError> {
    if value > U256::from(u32::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 32-bit integer"
        )));
    }

    Ok(value.as_u32())
}

fn validate_reserved_option(field: &str, value: Option<&Value>) -> Result<(), ValidationError> {
    if value.is_some() {
        return Err(ValidationError::not_supported(format!(
            "`options.{field}` is reserved and not supported yet"
        )));
    }

    Ok(())
}

fn map_block_ref(block: BlockRef) -> Result<service_espace::EspaceBlockRef, ValidationError> {
    match block {
        BlockRef::Tag(value) => match value.as_str() {
            "latest" => Ok(service_espace::EspaceBlockRef::Latest),
            value => Ok(service_espace::EspaceBlockRef::Number(parse_u64_quantity(
                value, "block",
            )?)),
        },
        BlockRef::Hash(_) => Err(ValidationError::not_supported(
            "eSpace block hash selectors are not supported yet",
        )),
    }
}
fn map_transaction(
    transaction: TransactionRequest,
) -> Result<service_espace::EspaceTransaction, ValidationError> {
    let tx_type = infer_transaction_type(&transaction)?;
    validate_transaction_shape(&transaction, tx_type)?;

    let from = require_transaction_from(&transaction)?;
    let nonce = require_transaction_field(transaction.nonce, "transaction.nonce")?;
    let gas_limit = require_transaction_gas(&transaction)?;
    let chain_id = u256_to_u32_quantity(
        require_transaction_field(transaction.chain_id, "transaction.chainId")?,
        "transaction.chainId",
    )?;

    let TransactionRequest {
        to,
        value,
        input,
        access_list,
        gas_price,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        ..
    } = transaction;

    let data: Bytes = input
        .try_into_unique_input()
        .map_err(|error| {
            ValidationError::invalid_params(format!("`transaction.input` is invalid: {error}"))
        })?
        .unwrap_or_default()
        .into();

    let access_list = access_list
        .unwrap_or_default()
        .into_iter()
        .map(|item| service_espace::AccessListItem {
            address: item.address,
            storage_keys: item.storage_keys,
        })
        .collect();

    let variant = match tx_type {
        EspaceTransactionType::Legacy => service_espace::EspaceTransactionVariant::Legacy {
            gas_price: require_transaction_field(gas_price, "transaction.gasPrice")?,
        },
        EspaceTransactionType::Eip2930 => service_espace::EspaceTransactionVariant::Eip2930 {
            gas_price: require_transaction_field(gas_price, "transaction.gasPrice")?,
            access_list,
        },
        EspaceTransactionType::Eip1559 => service_espace::EspaceTransactionVariant::Eip1559 {
            max_fee_per_gas: require_transaction_field(
                max_fee_per_gas,
                "transaction.maxFeePerGas",
            )?,
            max_priority_fee_per_gas: require_transaction_field(
                max_priority_fee_per_gas,
                "transaction.maxPriorityFeePerGas",
            )?,
            access_list,
        },
    };

    Ok(service_espace::EspaceTransaction {
        from,
        to,
        nonce,
        gas_limit,
        value: value.unwrap_or_default(),
        data,
        chain_id,
        variant,
    })
}

fn require_transaction_field<T>(value: Option<T>, field: &str) -> Result<T, ValidationError> {
    value.ok_or_else(|| ValidationError::invalid_params(format!("`{field}` is required")))
}

fn validate_transaction_shape(
    transaction: &TransactionRequest,
    tx_type: EspaceTransactionType,
) -> Result<(), ValidationError> {
    let has_dynamic_fee =
        transaction.max_fee_per_gas.is_some() || transaction.max_priority_fee_per_gas.is_some();

    match tx_type {
        EspaceTransactionType::Legacy => {
            if transaction.access_list.is_some() {
                return Err(ValidationError::invalid_params(
                    "legacy transactions cannot include `transaction.accessList`",
                ));
            }

            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "legacy transactions cannot include EIP-1559 fee fields",
                ));
            }
        }
        EspaceTransactionType::Eip2930 => {
            if has_dynamic_fee {
                return Err(ValidationError::invalid_params(
                    "EIP-2930 transactions cannot include EIP-1559 fee fields",
                ));
            }
        }
        EspaceTransactionType::Eip1559 => {
            if transaction.gas_price.is_some() {
                return Err(ValidationError::invalid_params(
                    "EIP-1559 transactions cannot include `transaction.gasPrice`",
                ));
            }
        }
    }

    Ok(())
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

fn infer_transaction_type(
    transaction: &TransactionRequest,
) -> Result<EspaceTransactionType, ValidationError> {
    match transaction.transaction_type.map(|value| value.as_u64()) {
        Some(0x0) => Ok(EspaceTransactionType::Legacy),
        Some(0x1) => Ok(EspaceTransactionType::Eip2930),
        Some(0x2) => Ok(EspaceTransactionType::Eip1559),
        Some(0x4) => Err(ValidationError::not_supported(
            "`transaction.type` `0x4` / EIP-7702 is not supported yet",
        )),
        Some(_) => Err(ValidationError::invalid_params(
            "`transaction.type` only supports `0x0`, `0x1`, and `0x2`",
        )),
        None if transaction.max_fee_per_gas.is_some()
            || transaction.max_priority_fee_per_gas.is_some() =>
        {
            Ok(EspaceTransactionType::Eip1559)
        }
        None if transaction.access_list.is_some() => Ok(EspaceTransactionType::Eip2930),
        None => Ok(EspaceTransactionType::Legacy),
    }
}
fn parse_u64_quantity(value: &str, field: &str) -> Result<u64, ValidationError> {
    let value = parse_quantity(value)?;

    if value > U256::from(u64::MAX) {
        return Err(ValidationError::invalid_params(format!(
            "`{field}` must fit into an unsigned 64-bit integer"
        )));
    }

    Ok(value.as_u64())
}

fn parse_quantity(value: &str) -> Result<U256, ValidationError> {
    let digits = value.strip_prefix("0x").ok_or_else(|| {
        ValidationError::invalid_params("quantity must be a 0x-prefixed hex string")
    })?;

    if digits.is_empty() {
        return Err(ValidationError::invalid_params(
            "quantity must not be empty",
        ));
    }

    if digits.len() > 1 && digits.starts_with('0') {
        return Err(ValidationError::invalid_params(
            "quantity must not contain leading zeroes",
        ));
    }

    let mut normalized = digits.to_string();
    if normalized.len() % 2 == 1 {
        normalized.insert(0, '0');
    }

    let bytes = hex::decode(&normalized)
        .map_err(|_| ValidationError::invalid_params("quantity must be a hex string"))?;

    if bytes.len() > 32 {
        return Err(ValidationError::invalid_params(
            "quantity must fit into an unsigned 256-bit integer",
        ));
    }

    Ok(U256::from_big_endian(&bytes))
}

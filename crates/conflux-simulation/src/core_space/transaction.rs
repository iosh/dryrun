use alloy_primitives::{B256, Bytes, U256};
use conflux_provider::{CoreAddress, Network};
use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};
use simulation_transaction::{TransactionType, TransactionVariantError};

use crate::{
    ConfluxSimulationError,
    execution::CoreSpaceTransactionInput,
    primitive::{b256_to_cfx, u256_to_cfx},
    state::{ConfluxSimulationProvider, ConfluxStateAnchor},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransactionRequest {
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub chain_id: u64,
    pub variant: CoreSpaceTransactionVariantRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceAccessListItem {
    pub address: CoreAddress,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceTransactionVariantRequest {
    Legacy {
        gas_price: Option<u128>,
    },
    AccessList {
        gas_price: Option<u128>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    DynamicFee {
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

impl CoreSpaceTransactionVariantRequest {
    pub fn try_new(
        transaction_type: TransactionType,
        access_list: Option<Vec<CoreSpaceAccessListItem>>,
        gas_price: Option<u128>,
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
    ) -> Result<Self, TransactionVariantError> {
        let has_dynamic_fee = max_fee_per_gas.is_some() || max_priority_fee_per_gas.is_some();

        match transaction_type {
            TransactionType::Legacy => {
                if access_list.as_ref().is_some_and(|items| !items.is_empty()) {
                    return Err(TransactionVariantError::AccessListNotAllowed { transaction_type });
                }

                if has_dynamic_fee {
                    return Err(TransactionVariantError::DynamicFeeNotAllowed { transaction_type });
                }

                Ok(Self::Legacy { gas_price })
            }
            TransactionType::AccessList => {
                if has_dynamic_fee {
                    return Err(TransactionVariantError::DynamicFeeNotAllowed { transaction_type });
                }

                Ok(Self::AccessList {
                    gas_price,
                    access_list: access_list.unwrap_or_default(),
                })
            }
            TransactionType::DynamicFee => {
                if gas_price.is_some() {
                    return Err(TransactionVariantError::GasPriceNotAllowed { transaction_type });
                }

                Ok(Self::DynamicFee {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    access_list: access_list.unwrap_or_default(),
                })
            }
        }
    }
}

pub(crate) fn validate_core_space_transaction_network(
    transaction: &CoreSpaceTransactionRequest,
    expected_network: Network,
) -> Result<(), ConfluxSimulationError> {
    validate_core_space_address_network(&transaction.from, expected_network, "transaction.from")?;

    if let Some(to) = transaction.to.as_ref() {
        validate_core_space_address_network(to, expected_network, "transaction.to")?;
    }

    let access_list = match &transaction.variant {
        CoreSpaceTransactionVariantRequest::Legacy { .. } => None,
        CoreSpaceTransactionVariantRequest::AccessList { access_list, .. }
        | CoreSpaceTransactionVariantRequest::DynamicFee { access_list, .. } => Some(access_list),
    };
    if let Some(access_list) = access_list {
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
    address: &CoreAddress,
    expected_network: Network,
    field: &str,
) -> Result<(), ConfluxSimulationError> {
    if address.network() != expected_network {
        return Err(ConfluxSimulationError::transaction_completion_failed(
            format!(
                "`{field}` uses address network {}, expected {}",
                address.network(),
                expected_network
            ),
        ));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceTransactionVariant {
    Legacy {
        gas_price: u128,
    },
    AccessList {
        gas_price: u128,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    DynamicFee {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransaction {
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u64,
    pub variant: CoreSpaceTransactionVariant,
    pub storage_limit: u64,
    pub epoch_height: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedStoragePayer {
    storage_covered_by_sponsor: bool,
}

impl PreparedStoragePayer {
    pub(crate) const fn storage_covered_by_sponsor(self) -> bool {
        self.storage_covered_by_sponsor
    }
}

pub(crate) async fn prepare_storage_payer(
    provider: &ConfluxSimulationProvider,
    state_anchor: ConfluxStateAnchor,
    transaction: &CoreSpaceTransaction,
) -> Result<PreparedStoragePayer, ConfluxSimulationError> {
    let Some(target) = transaction.to.as_ref() else {
        return Ok(PreparedStoragePayer {
            storage_covered_by_sponsor: false,
        });
    };

    let target_cfx = cfx_types::Address::from_slice(&target.bytes());
    let code = provider
        .cfx_get_code(target_cfx, state_anchor.core_space_pivot())
        .await?;
    if code.is_empty() {
        return Ok(PreparedStoragePayer {
            storage_covered_by_sponsor: false,
        });
    }

    let storage_limit = transaction.storage_limit;
    let balance_check = provider
        .cfx_check_balance_against_transaction(
            transaction.from,
            *target,
            transaction.gas_limit,
            storage_payer_gas_price(&transaction.variant),
            storage_limit,
            state_anchor.core_space_epoch(),
        )
        .await?;

    Ok(PreparedStoragePayer {
        storage_covered_by_sponsor: !balance_check.will_pay_collateral,
    })
}

fn storage_payer_gas_price(variant: &CoreSpaceTransactionVariant) -> u128 {
    match variant {
        CoreSpaceTransactionVariant::Legacy { gas_price }
        | CoreSpaceTransactionVariant::AccessList { gas_price, .. } => *gas_price,
        CoreSpaceTransactionVariant::DynamicFee {
            max_fee_per_gas, ..
        } => *max_fee_per_gas,
    }
}

pub(crate) fn build_core_space_transaction_input(
    input: CoreSpaceTransaction,
    chain_id: u32,
) -> CoreSpaceTransactionInput {
    let sender = cfx_types::Address::from_slice(&input.from.bytes());
    let tx = build_typed_core_space_transaction(input, chain_id);

    CoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(
    input: CoreSpaceTransaction,
    chain_id: u32,
) -> TypedNativeTransaction {
    let CoreSpaceTransaction {
        from: _,
        to,
        nonce,
        gas_limit,
        value,
        data,
        chain_id: _,
        variant,
        storage_limit,
        epoch_height,
    } = input;

    let action = to.map_or(Action::Create, |address| {
        Action::Call(cfx_types::Address::from_slice(&address.bytes()))
    });
    let nonce = nonce.into();
    let gas = gas_limit.into();
    let value = u256_to_cfx(value);
    let data = data.to_vec();

    match variant {
        CoreSpaceTransactionVariant::Legacy { gas_price } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price: gas_price.into(),
                gas,
                action,
                value,
                storage_limit,
                epoch_height,
                chain_id,
                data,
            })
        }
        CoreSpaceTransactionVariant::AccessList {
            gas_price,
            access_list,
        } => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price: gas_price.into(),
            gas,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: core_access_list_to_cfx(access_list),
        }),
        CoreSpaceTransactionVariant::DynamicFee {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas: max_priority_fee_per_gas.into(),
            max_fee_per_gas: max_fee_per_gas.into(),
            gas,
            action,
            value,
            storage_limit,
            epoch_height,
            chain_id,
            data,
            access_list: core_access_list_to_cfx(access_list),
        }),
    }
}

fn core_access_list_to_cfx(items: Vec<CoreSpaceAccessListItem>) -> Vec<primitives::AccessListItem> {
    items
        .into_iter()
        .map(|item| primitives::AccessListItem {
            address: cfx_types::Address::from_slice(&item.address.bytes()),
            storage_keys: item.storage_keys.into_iter().map(b256_to_cfx).collect(),
        })
        .collect()
}

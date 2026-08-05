use alloy_primitives::{Address, B256, Bytes, U256};
use cfx_rpc_cfx_types::EpochNumber;
use conflux_provider::{CoreAddress, Network};
use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};
use simulation_transaction::{
    TransactionRequest, TransactionType, TransactionVariantError, TransactionVariantRequest,
};

use crate::{
    ConfluxSimulationError, ConfluxTransaction, ConfluxTransactionVariant,
    execution::CoreSpaceTransactionInput,
    primitive::{access_list_to_cfx, address_to_cfx, u256_to_cfx},
    state::ConfluxSimulationProvider,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceEpochRef {
    LatestState,
    Number(u64),
}

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

    fn into_shared(self) -> TransactionVariantRequest {
        match self {
            Self::Legacy { gas_price } => TransactionVariantRequest::Legacy { gas_price },
            Self::AccessList {
                gas_price,
                access_list,
            } => TransactionVariantRequest::AccessList {
                gas_price,
                access_list: access_list
                    .into_iter()
                    .map(|item| simulation_transaction::AccessListItem {
                        address: core_address_to_alloy(item.address),
                        storage_keys: item.storage_keys,
                    })
                    .collect(),
            },
            Self::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list,
            } => TransactionVariantRequest::DynamicFee {
                max_fee_per_gas,
                max_priority_fee_per_gas,
                access_list: access_list
                    .into_iter()
                    .map(|item| simulation_transaction::AccessListItem {
                        address: core_address_to_alloy(item.address),
                        storage_keys: item.storage_keys,
                    })
                    .collect(),
            },
        }
    }
}

impl CoreSpaceTransactionRequest {
    pub(crate) fn into_shared(self) -> TransactionRequest {
        TransactionRequest {
            from: core_address_to_alloy(self.from),
            to: self.to.map(core_address_to_alloy),
            nonce: self.nonce,
            gas_limit: self.gas_limit,
            value: self.value,
            data: self.data,
            chain_id: self.chain_id,
            variant: self.variant.into_shared(),
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

fn core_address_to_alloy(address: CoreAddress) -> Address {
    Address::from_slice(&address.bytes())
}

pub type CoreSpaceTransactionVariant = ConfluxTransactionVariant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceTransaction {
    pub transaction: ConfluxTransaction,
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
    epoch: EpochNumber,
    transaction: &CoreSpaceTransaction,
) -> Result<PreparedStoragePayer, ConfluxSimulationError> {
    let Some(target) = transaction.transaction.to else {
        return Ok(PreparedStoragePayer {
            storage_covered_by_sponsor: false,
        });
    };

    let target = address_to_cfx(target);
    let code = provider.cfx_get_code(target, epoch.clone()).await?;
    if code.is_empty() {
        return Ok(PreparedStoragePayer {
            storage_covered_by_sponsor: false,
        });
    }

    let storage_limit = transaction.storage_limit;
    let transaction = &transaction.transaction;
    let balance_check = provider
        .cfx_check_balance_against_transaction(
            address_to_cfx(transaction.from),
            target,
            transaction.gas_limit,
            storage_payer_gas_price(&transaction.variant),
            storage_limit,
            epoch,
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
    let sender = address_to_cfx(input.transaction.from);
    let tx = build_typed_core_space_transaction(input, chain_id);

    CoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(
    input: CoreSpaceTransaction,
    chain_id: u32,
) -> TypedNativeTransaction {
    let CoreSpaceTransaction {
        transaction,
        storage_limit,
        epoch_height,
    } = input;
    let ConfluxTransaction {
        to,
        nonce,
        gas_limit,
        value,
        data,
        variant,
        ..
    } = transaction;

    let action = to.map_or(Action::Create, |address| {
        Action::Call(address_to_cfx(address))
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
            access_list: access_list_to_cfx(access_list),
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
            access_list: access_list_to_cfx(access_list),
        }),
    }
}

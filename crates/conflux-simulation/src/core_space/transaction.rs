use alloy_primitives::{Bytes, U256};
use conflux_provider::{CoreAddress, Network};
use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};
#[cfg(feature = "serde")]
use serde::Serialize;
pub use simulation_core::transaction::DynamicFees;
use simulation_core::transaction::TransactionCommon;
use thiserror::Error;

use crate::{
    ConfluxRpcError,
    execution::CoreSpaceTransactionInput as ExecutorCoreSpaceTransactionInput,
    primitive::{b256_to_cfx, u256_to_cfx},
    state::{ConfluxSimulationProvider, ConfluxStateAnchor},
};

pub type CoreSpaceTransactionCommon = TransactionCommon<CoreAddress, U256, u32>;
pub type CoreSpaceAccessListItem = simulation_core::transaction::AccessListItem<CoreAddress>;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", rename_all_fields = "camelCase")
)]
pub enum CoreSpaceTypedTransaction {
    #[cfg_attr(feature = "serde", serde(rename = "0x0"))]
    Cip155 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: CoreSpaceTransactionCommon,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        storage_limit: u64,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        epoch_height: u64,
        gas_price: U256,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x1"))]
    Cip2930 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: CoreSpaceTransactionCommon,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        storage_limit: u64,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        epoch_height: u64,
        gas_price: U256,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x2"))]
    Cip1559 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: CoreSpaceTransactionCommon,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        storage_limit: u64,
        #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
        epoch_height: u64,
        #[cfg_attr(feature = "serde", serde(flatten))]
        fees: DynamicFees,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CoreSpaceTransactionType {
    Cip155 = 0,
    Cip2930 = 1,
    Cip1559 = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceTransactionInput {
    Complete(CoreSpaceTypedTransaction),
    Partial(CoreSpacePartialTransaction),
}

impl CoreSpaceTransactionInput {
    pub(crate) fn validate_network(
        &self,
        expected: Network,
    ) -> Result<(), CoreSpaceTransactionInputError> {
        match self {
            Self::Complete(transaction) => transaction.validate_network(expected),
            Self::Partial(transaction) => transaction.validate_network(expected),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpacePartialTransactionCommon {
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub nonce: Option<U256>,
    pub gas_limit: Option<U256>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub chain_id: Option<u32>,
    pub storage_limit: Option<u64>,
    pub epoch_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpacePartialTransaction {
    Cip155 {
        common: CoreSpacePartialTransactionCommon,
        gas_price: Option<U256>,
    },
    Cip2930 {
        common: CoreSpacePartialTransactionCommon,
        gas_price: Option<U256>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    Cip1559 {
        common: CoreSpacePartialTransactionCommon,
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

impl CoreSpaceTypedTransaction {
    pub fn common(&self) -> &CoreSpaceTransactionCommon {
        match self {
            Self::Cip155 { common, .. }
            | Self::Cip2930 { common, .. }
            | Self::Cip1559 { common, .. } => common,
        }
    }

    pub fn common_mut(&mut self) -> &mut CoreSpaceTransactionCommon {
        match self {
            Self::Cip155 { common, .. }
            | Self::Cip2930 { common, .. }
            | Self::Cip1559 { common, .. } => common,
        }
    }

    pub fn storage_limit(&self) -> u64 {
        match self {
            Self::Cip155 { storage_limit, .. }
            | Self::Cip2930 { storage_limit, .. }
            | Self::Cip1559 { storage_limit, .. } => *storage_limit,
        }
    }

    pub fn epoch_height(&self) -> u64 {
        match self {
            Self::Cip155 { epoch_height, .. }
            | Self::Cip2930 { epoch_height, .. }
            | Self::Cip1559 { epoch_height, .. } => *epoch_height,
        }
    }

    pub fn transaction_type(&self) -> CoreSpaceTransactionType {
        match self {
            Self::Cip155 { .. } => CoreSpaceTransactionType::Cip155,
            Self::Cip2930 { .. } => CoreSpaceTransactionType::Cip2930,
            Self::Cip1559 { .. } => CoreSpaceTransactionType::Cip1559,
        }
    }

    pub(crate) fn access_list(&self) -> Option<&[CoreSpaceAccessListItem]> {
        match self {
            Self::Cip155 { .. } => None,
            Self::Cip2930 { access_list, .. } | Self::Cip1559 { access_list, .. } => {
                Some(access_list)
            }
        }
    }

    pub(crate) fn gas_price_for_sponsorship_check(&self) -> U256 {
        match self {
            Self::Cip155 { gas_price, .. } | Self::Cip2930 { gas_price, .. } => *gas_price,
            Self::Cip1559 { fees, .. } => fees.max_fee_per_gas,
        }
    }

    fn validate_network(&self, expected: Network) -> Result<(), CoreSpaceTransactionInputError> {
        let common = self.common();
        validate_transaction_network(common.from, common.to, self.access_list(), expected)
    }
}

impl CoreSpacePartialTransaction {
    fn common(&self) -> &CoreSpacePartialTransactionCommon {
        match self {
            Self::Cip155 { common, .. }
            | Self::Cip2930 { common, .. }
            | Self::Cip1559 { common, .. } => common,
        }
    }

    fn access_list(&self) -> Option<&[CoreSpaceAccessListItem]> {
        match self {
            Self::Cip155 { .. } => None,
            Self::Cip2930 { access_list, .. } | Self::Cip1559 { access_list, .. } => {
                Some(access_list)
            }
        }
    }

    fn validate_network(&self, expected: Network) -> Result<(), CoreSpaceTransactionInputError> {
        let common = self.common();
        validate_transaction_network(common.from, common.to, self.access_list(), expected)
    }
}

fn validate_transaction_network(
    from: CoreAddress,
    to: Option<CoreAddress>,
    access_list: Option<&[CoreSpaceAccessListItem]>,
    expected: Network,
) -> Result<(), CoreSpaceTransactionInputError> {
    validate_address_network(from, expected)?;
    if let Some(to) = to {
        validate_address_network(to, expected)?;
    }
    if let Some(access_list) = access_list {
        for item in access_list {
            validate_address_network(item.address, expected)?;
        }
    }

    Ok(())
}

fn validate_address_network(
    address: CoreAddress,
    expected: Network,
) -> Result<(), CoreSpaceTransactionInputError> {
    if address.network() != expected {
        return Err(CoreSpaceTransactionInputError::AddressNetworkMismatch { address, expected });
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CoreSpaceTransactionInputError {
    #[error(
        "Core Space address {address:?} uses network {}, expected {expected}",
        address.network()
    )]
    AddressNetworkMismatch {
        address: CoreAddress,
        expected: Network,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceTransactionCompletionError {
    #[error(transparent)]
    Provider(#[from] ConfluxRpcError),

    #[error("estimated Core Space storage limit exceeds u64: {value}")]
    StorageLimitOutOfRange { value: U256 },

    #[error("Core Space epoch {epoch_number} has no base fee for dynamic-fee completion")]
    MissingBaseFee { epoch_number: u64 },

    #[error("calculated Core Space max fee per gas exceeds U256")]
    MaxFeePerGasOverflow,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StorageSponsorship {
    storage_covered_by_sponsor: bool,
}

impl StorageSponsorship {
    pub(crate) const fn storage_covered_by_sponsor(self) -> bool {
        self.storage_covered_by_sponsor
    }
}

pub(crate) async fn check_storage_sponsorship(
    provider: &ConfluxSimulationProvider,
    state_anchor: ConfluxStateAnchor,
    transaction: &CoreSpaceTypedTransaction,
) -> Result<StorageSponsorship, super::CoreSpaceExecutionError> {
    let common = transaction.common();
    let Some(target) = common.to.as_ref() else {
        return Ok(StorageSponsorship {
            storage_covered_by_sponsor: false,
        });
    };

    let target_cfx = cfx_types::Address::from_slice(&target.bytes());
    let code = provider
        .cfx_get_code(target_cfx, state_anchor.core_space_pivot())
        .await
        .map_err(|source| {
            super::CoreSpaceExecutionError::StateAccess(
                super::CoreSpaceStateAccessError::Provider { source },
            )
        })?;
    if code.is_empty() {
        return Ok(StorageSponsorship {
            storage_covered_by_sponsor: false,
        });
    }

    let storage_limit = transaction.storage_limit();
    let balance_check = provider
        .cfx_check_balance_against_transaction(
            common.from,
            *target,
            common.gas_limit,
            transaction.gas_price_for_sponsorship_check(),
            storage_limit,
            state_anchor.core_space_epoch(),
        )
        .await
        .map_err(|source| {
            super::CoreSpaceExecutionError::StateAccess(
                super::CoreSpaceStateAccessError::Provider { source },
            )
        })?;

    Ok(StorageSponsorship {
        storage_covered_by_sponsor: !balance_check.will_pay_collateral,
    })
}

pub(super) fn build_core_space_transaction_input(
    input: &CoreSpaceTypedTransaction,
    chain_id: u32,
) -> ExecutorCoreSpaceTransactionInput {
    let sender = cfx_types::Address::from_slice(&input.common().from.bytes());
    let tx = to_native_transaction(input, chain_id);

    ExecutorCoreSpaceTransactionInput { tx, sender }
}

fn to_native_transaction(
    input: &CoreSpaceTypedTransaction,
    chain_id: u32,
) -> TypedNativeTransaction {
    let common = input.common();

    let action = common.to.as_ref().map_or(Action::Create, |address| {
        Action::Call(cfx_types::Address::from_slice(&address.bytes()))
    });
    let nonce = u256_to_cfx(common.nonce);
    let gas = u256_to_cfx(common.gas_limit);
    let value = u256_to_cfx(common.value);
    let data = common.input.to_vec();

    match input {
        CoreSpaceTypedTransaction::Cip155 { gas_price, .. } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price: u256_to_cfx(*gas_price),
                gas,
                action,
                value,
                storage_limit: input.storage_limit(),
                epoch_height: input.epoch_height(),
                chain_id,
                data,
            })
        }
        CoreSpaceTypedTransaction::Cip2930 {
            gas_price,
            access_list,
            ..
        } => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price: u256_to_cfx(*gas_price),
            gas,
            action,
            value,
            storage_limit: input.storage_limit(),
            epoch_height: input.epoch_height(),
            chain_id,
            data,
            access_list: core_access_list_to_cfx(access_list),
        }),
        CoreSpaceTypedTransaction::Cip1559 {
            fees:
                DynamicFees {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                },
            access_list,
            ..
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            action,
            value,
            storage_limit: input.storage_limit(),
            epoch_height: input.epoch_height(),
            chain_id,
            data,
            access_list: core_access_list_to_cfx(access_list),
        }),
    }
}

fn core_access_list_to_cfx(items: &[CoreSpaceAccessListItem]) -> Vec<primitives::AccessListItem> {
    items
        .iter()
        .map(|item| primitives::AccessListItem {
            address: cfx_types::Address::from_slice(&item.address.bytes()),
            storage_keys: item.storage_keys.iter().copied().map(b256_to_cfx).collect(),
        })
        .collect()
}

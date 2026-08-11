use alloy_primitives::{B256, Bytes, U256};
use conflux_provider::{CoreAddress, Network};
use primitives::transaction::{
    Action, Cip1559Transaction, Cip2930Transaction,
    NativeTransaction as PrimitiveNativeTransaction, TypedNativeTransaction,
};
use thiserror::Error;

use crate::{
    ConfluxRpcError, ConfluxSimulationError,
    execution::CoreSpaceTransactionInput as ExecutorCoreSpaceTransactionInput,
    primitive::{b256_to_cfx, u256_to_cfx},
    state::{ConfluxSimulationProvider, ConfluxStateAnchor},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceTransactionInput {
    Complete(CoreSpaceCompleteTransaction),
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
pub struct CoreSpaceCompleteTransaction {
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u32,
    pub variant: CoreSpaceCompleteTransactionVariant,
    pub storage_limit: u64,
    pub epoch_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpaceAccessListItem {
    pub address: CoreAddress,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpaceCompleteTransactionVariant {
    Cip155 {
        gas_price: U256,
    },
    Cip2930 {
        gas_price: U256,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    Cip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpacePartialTransaction {
    pub from: CoreAddress,
    pub to: Option<CoreAddress>,
    pub nonce: Option<U256>,
    pub gas_limit: Option<U256>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub chain_id: Option<u32>,
    pub variant: CoreSpacePartialTransactionVariant,
    pub storage_limit: Option<u64>,
    pub epoch_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreSpacePartialTransactionVariant {
    Cip155 {
        gas_price: Option<U256>,
    },
    Cip2930 {
        gas_price: Option<U256>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
    Cip1559 {
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        access_list: Vec<CoreSpaceAccessListItem>,
    },
}

impl CoreSpaceCompleteTransaction {
    fn validate_network(&self, expected: Network) -> Result<(), CoreSpaceTransactionInputError> {
        validate_transaction_network(
            self.from,
            self.to,
            complete_access_list(&self.variant),
            expected,
        )
    }
}

impl CoreSpacePartialTransaction {
    fn validate_network(&self, expected: Network) -> Result<(), CoreSpaceTransactionInputError> {
        validate_transaction_network(
            self.from,
            self.to,
            partial_access_list(&self.variant),
            expected,
        )
    }
}

fn complete_access_list(
    variant: &CoreSpaceCompleteTransactionVariant,
) -> Option<&[CoreSpaceAccessListItem]> {
    match variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { .. } => None,
        CoreSpaceCompleteTransactionVariant::Cip2930 { access_list, .. }
        | CoreSpaceCompleteTransactionVariant::Cip1559 { access_list, .. } => Some(access_list),
    }
}

fn partial_access_list(
    variant: &CoreSpacePartialTransactionVariant,
) -> Option<&[CoreSpaceAccessListItem]> {
    match variant {
        CoreSpacePartialTransactionVariant::Cip155 { .. } => None,
        CoreSpacePartialTransactionVariant::Cip2930 { access_list, .. }
        | CoreSpacePartialTransactionVariant::Cip1559 { access_list, .. } => Some(access_list),
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
    transaction: &CoreSpaceCompleteTransaction,
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

fn storage_payer_gas_price(variant: &CoreSpaceCompleteTransactionVariant) -> U256 {
    match variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { gas_price }
        | CoreSpaceCompleteTransactionVariant::Cip2930 { gas_price, .. } => *gas_price,
        CoreSpaceCompleteTransactionVariant::Cip1559 {
            max_fee_per_gas, ..
        } => *max_fee_per_gas,
    }
}

pub(super) fn build_core_space_transaction_input(
    input: &CoreSpaceCompleteTransaction,
    chain_id: u32,
) -> ExecutorCoreSpaceTransactionInput {
    let sender = cfx_types::Address::from_slice(&input.from.bytes());
    let tx = build_typed_core_space_transaction(input, chain_id);

    ExecutorCoreSpaceTransactionInput { tx, sender }
}

fn build_typed_core_space_transaction(
    input: &CoreSpaceCompleteTransaction,
    chain_id: u32,
) -> TypedNativeTransaction {
    let CoreSpaceCompleteTransaction {
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

    let action = to.as_ref().map_or(Action::Create, |address| {
        Action::Call(cfx_types::Address::from_slice(&address.bytes()))
    });
    let nonce = u256_to_cfx(*nonce);
    let gas = u256_to_cfx(*gas_limit);
    let value = u256_to_cfx(*value);
    let data = data.to_vec();

    match variant {
        CoreSpaceCompleteTransactionVariant::Cip155 { gas_price } => {
            TypedNativeTransaction::Cip155(PrimitiveNativeTransaction {
                nonce,
                gas_price: u256_to_cfx(*gas_price),
                gas,
                action,
                value,
                storage_limit: *storage_limit,
                epoch_height: *epoch_height,
                chain_id,
                data,
            })
        }
        CoreSpaceCompleteTransactionVariant::Cip2930 {
            gas_price,
            access_list,
        } => TypedNativeTransaction::Cip2930(Cip2930Transaction {
            nonce,
            gas_price: u256_to_cfx(*gas_price),
            gas,
            action,
            value,
            storage_limit: *storage_limit,
            epoch_height: *epoch_height,
            chain_id,
            data,
            access_list: core_access_list_to_cfx(access_list),
        }),
        CoreSpaceCompleteTransactionVariant::Cip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            access_list,
        } => TypedNativeTransaction::Cip1559(Cip1559Transaction {
            nonce,
            max_priority_fee_per_gas: u256_to_cfx(*max_priority_fee_per_gas),
            max_fee_per_gas: u256_to_cfx(*max_fee_per_gas),
            gas,
            action,
            value,
            storage_limit: *storage_limit,
            epoch_height: *epoch_height,
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

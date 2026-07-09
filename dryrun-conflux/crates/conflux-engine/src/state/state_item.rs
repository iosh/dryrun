use std::fmt;

use crate::state::native_internal::{
    NativeInternalStateItem, parse_native_internal_storage,
};
use cfx_statedb::global_params::{
    AccumulateInterestRate, BaseFeeProp, ConvertedStoragePoints, DistributablePoSInterest,
    GlobalParamKey, InterestRate, LastDistributeBlock, PowBaseReward, TotalBurnt1559,
    TotalEvmToken, TotalIssued, TotalPosStaking, TotalStaking, TotalStorage, UsedStoragePoints,
};
use cfx_types::{Address, H256, Space};
use primitives::{StorageKey, StorageKeyWithSpace};
use thiserror::Error;

const ADDRESS_BYTES: usize = StorageKeyWithSpace::ACCOUNT_BYTES;
const HASH_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateItem {
    Native(NativeStateItem),
    Espace(EspaceStateItem),
}

impl StateItem {
    pub(crate) fn from_storage_key(
        storage_key: StorageKeyWithSpace<'_>,
    ) -> Result<Self, StateItemError> {
        match storage_key.space {
            Space::Ethereum => from_espace_key(storage_key.key).map(Self::Espace),
            Space::Native => from_native_key(storage_key).map(Self::Native),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeStateItem {
    TotalIssued,
    TotalStaking,
    InterestRate,
    TotalEvmToken,
    TotalStorage,
    UsedStoragePoints,
    ConvertedStoragePoints,
    AccumulateInterestRate,
    TotalPosStaking,
    DistributablePosInterest,
    LastDistributeBlock,
    PowBaseReward,
    TotalBurnt1559,
    BaseFeeProp,
    Account { address: Address },
    StorageSlot { address: Address, slot: H256 },
    InternalContractStorage(NativeInternalStateItem),
    Code { address: Address, code_hash: H256 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EspaceStateItem {
    Account { address: Address },
    StorageSlot { address: Address, slot: H256 },
    Code { address: Address, code_hash: H256 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageKeyKind {
    Account,
    StorageRoot,
    Storage,
    CodeRoot,
    Code,
    DepositList,
    VoteList,
    Empty,
    AddressPrefix,
}

impl StorageKeyKind {
    fn from_storage_key(key: StorageKey<'_>) -> Self {
        match key {
            StorageKey::AccountKey(_) => Self::Account,
            StorageKey::StorageRootKey(_) => Self::StorageRoot,
            StorageKey::StorageKey { .. } => Self::Storage,
            StorageKey::CodeRootKey(_) => Self::CodeRoot,
            StorageKey::CodeKey { .. } => Self::Code,
            StorageKey::DepositListKey(_) => Self::DepositList,
            StorageKey::VoteListKey(_) => Self::VoteList,
            StorageKey::EmptyKey => Self::Empty,
            StorageKey::AddressPrefixKey(_) => Self::AddressPrefix,
        }
    }
}

impl fmt::Display for StorageKeyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Account => "AccountKey",
            Self::StorageRoot => "StorageRootKey",
            Self::Storage => "StorageKey",
            Self::CodeRoot => "CodeRootKey",
            Self::Code => "CodeKey",
            Self::DepositList => "DepositListKey",
            Self::VoteList => "VoteListKey",
            Self::Empty => "EmptyKey",
            Self::AddressPrefix => "AddressPrefixKey",
        };

        f.write_str(kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum StateItemError {
    #[error("unsupported native state key")]
    UnsupportedNativeKey,
    #[error("unsupported eSpace storage key kind: {kind}")]
    UnsupportedEspaceKey { kind: StorageKeyKind },
    #[error("invalid address length: expected {ADDRESS_BYTES} bytes, got {actual}")]
    InvalidAddressLength { actual: usize },
    #[error("invalid storage slot length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidStorageSlotLength { actual: usize },
    #[error("invalid code hash length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidCodeHashLength { actual: usize },
}

fn from_native_key(
    storage_key: StorageKeyWithSpace<'_>,
) -> Result<NativeStateItem, StateItemError> {
    if let StorageKey::AccountKey(address_bytes) = storage_key.key {
        return Ok(NativeStateItem::Account {
            address: parse_address(address_bytes)?,
        });
    }

    if storage_key == <InterestRate as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::InterestRate);
    }

    if storage_key == <AccumulateInterestRate as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::AccumulateInterestRate);
    }

    if storage_key == <TotalIssued as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalIssued);
    }

    if storage_key == <TotalStaking as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalStaking);
    }

    if storage_key == <TotalEvmToken as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalEvmToken);
    }

    if storage_key == <TotalStorage as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalStorage);
    }

    if storage_key == <UsedStoragePoints as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::UsedStoragePoints);
    }

    if storage_key == <ConvertedStoragePoints as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::ConvertedStoragePoints);
    }

    if storage_key == <TotalPosStaking as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalPosStaking);
    }

    if storage_key == <DistributablePoSInterest as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::DistributablePosInterest);
    }

    if storage_key == <LastDistributeBlock as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::LastDistributeBlock);
    }

    if storage_key == <PowBaseReward as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::PowBaseReward);
    }

    if storage_key == <TotalBurnt1559 as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::TotalBurnt1559);
    }

    if storage_key == <BaseFeeProp as GlobalParamKey>::STORAGE_KEY {
        return Ok(NativeStateItem::BaseFeeProp);
    }

    if let StorageKey::StorageKey {
        address_bytes,
        storage_key,
    } = storage_key.key
    {
        let address = parse_address(address_bytes)?;
        if let Some(item) = parse_native_internal_storage(address, storage_key) {
            return Ok(NativeStateItem::InternalContractStorage(item));
        }

        return Ok(NativeStateItem::StorageSlot {
            address,
            slot: parse_storage_slot(storage_key)?,
        });
    }

    if let StorageKey::CodeKey {
        address_bytes,
        code_hash_bytes,
    } = storage_key.key
    {
        return Ok(NativeStateItem::Code {
            address: parse_address(address_bytes)?,
            code_hash: parse_code_hash(code_hash_bytes)?,
        });
    }

    Err(StateItemError::UnsupportedNativeKey)
}

fn from_espace_key(key: StorageKey<'_>) -> Result<EspaceStateItem, StateItemError> {
    match key {
        StorageKey::AccountKey(address_bytes) => Ok(EspaceStateItem::Account {
            address: parse_address(address_bytes)?,
        }),
        StorageKey::StorageKey {
            address_bytes,
            storage_key,
        } => Ok(EspaceStateItem::StorageSlot {
            address: parse_address(address_bytes)?,
            slot: parse_storage_slot(storage_key)?,
        }),
        StorageKey::CodeKey {
            address_bytes,
            code_hash_bytes,
        } => Ok(EspaceStateItem::Code {
            address: parse_address(address_bytes)?,
            code_hash: parse_code_hash(code_hash_bytes)?,
        }),
        other => Err(StateItemError::UnsupportedEspaceKey {
            kind: StorageKeyKind::from_storage_key(other),
        }),
    }
}

fn parse_address(address_bytes: &[u8]) -> Result<Address, StateItemError> {
    if address_bytes.len() != ADDRESS_BYTES {
        return Err(StateItemError::InvalidAddressLength {
            actual: address_bytes.len(),
        });
    }

    Ok(Address::from_slice(address_bytes))
}

fn parse_storage_slot(slot_bytes: &[u8]) -> Result<H256, StateItemError> {
    if slot_bytes.len() != HASH_BYTES {
        return Err(StateItemError::InvalidStorageSlotLength {
            actual: slot_bytes.len(),
        });
    }

    Ok(H256::from_slice(slot_bytes))
}

fn parse_code_hash(code_hash_bytes: &[u8]) -> Result<H256, StateItemError> {
    if code_hash_bytes.len() != HASH_BYTES {
        return Err(StateItemError::InvalidCodeHashLength {
            actual: code_hash_bytes.len(),
        });
    }

    Ok(H256::from_slice(code_hash_bytes))
}

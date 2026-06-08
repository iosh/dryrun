use std::fmt;

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
pub(crate) enum StateReadRequest {
    NativeTotalIssued,
    NativeTotalStaking,
    NativeInterestRate,
    NativeTotalEvmToken,
    NativeTotalStorage,
    NativeUsedStoragePoints,
    NativeConvertedStoragePoints,
    NativeAccumulateInterestRate,
    NativeTotalPosStaking,
    NativeDistributablePosInterest,
    NativeLastDistributeBlock,
    NativePowBaseReward,
    NativeTotalBurnt1559,
    NativeBaseFeeProp,

    EspaceAccount { address: Address },
    EspaceStorageSlot { address: Address, slot: H256 },
    EspaceCode { address: Address, code_hash: H256 },
}

impl StateReadRequest {
    pub(crate) fn from_storage_key(
        storage_key: StorageKeyWithSpace<'_>,
    ) -> Result<Self, StateReadRequestError> {
        match storage_key.space {
            Space::Ethereum => from_espace_key(storage_key.key),
            Space::Native => from_native_key(storage_key),
        }
    }
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
pub(crate) enum StateReadRequestError {
    #[error("unsupported native state key")]
    UnsupportedNativeKey,
    #[error("unsupported eSpace storage key kind: {kind}")]
    UnsupportedEspaceKey { kind: StorageKeyKind },
    #[error("invalid address length: expected {ADDRESS_BYTES} bytes, got {actual}")]
    InvalidAddressLength { actual: usize },
    #[error("invalid eSpace storage slot length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidEspaceStorageSlotLength { actual: usize },
    #[error("invalid eSpace code hash length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidEspaceCodeHashLength { actual: usize },
}

fn from_native_key(
    storage_key: StorageKeyWithSpace<'_>,
) -> Result<StateReadRequest, StateReadRequestError> {
    if storage_key == <InterestRate as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeInterestRate);
    }

    if storage_key == <AccumulateInterestRate as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeAccumulateInterestRate);
    }

    if storage_key == <TotalIssued as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalIssued);
    }

    if storage_key == <TotalStaking as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalStaking);
    }

    if storage_key == <TotalEvmToken as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalEvmToken);
    }

    if storage_key == <TotalStorage as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalStorage);
    }

    if storage_key == <UsedStoragePoints as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeUsedStoragePoints);
    }

    if storage_key == <ConvertedStoragePoints as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeConvertedStoragePoints);
    }

    if storage_key == <TotalPosStaking as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalPosStaking);
    }

    if storage_key == <DistributablePoSInterest as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeDistributablePosInterest);
    }

    if storage_key == <LastDistributeBlock as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeLastDistributeBlock);
    }

    if storage_key == <PowBaseReward as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativePowBaseReward);
    }

    if storage_key == <TotalBurnt1559 as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeTotalBurnt1559);
    }

    if storage_key == <BaseFeeProp as GlobalParamKey>::STORAGE_KEY {
        return Ok(StateReadRequest::NativeBaseFeeProp);
    }

    Err(StateReadRequestError::UnsupportedNativeKey)
}
fn from_espace_key(key: StorageKey<'_>) -> Result<StateReadRequest, StateReadRequestError> {
    match key {
        StorageKey::AccountKey(address_bytes) => Ok(StateReadRequest::EspaceAccount {
            address: parse_address(address_bytes)?,
        }),
        StorageKey::StorageKey {
            address_bytes,
            storage_key,
        } => Ok(StateReadRequest::EspaceStorageSlot {
            address: parse_address(address_bytes)?,
            slot: parse_storage_slot(storage_key)?,
        }),
        StorageKey::CodeKey {
            address_bytes,
            code_hash_bytes,
        } => Ok(StateReadRequest::EspaceCode {
            address: parse_address(address_bytes)?,
            code_hash: parse_code_hash(code_hash_bytes)?,
        }),
        other => Err(StateReadRequestError::UnsupportedEspaceKey {
            kind: StorageKeyKind::from_storage_key(other),
        }),
    }
}

fn parse_address(address_bytes: &[u8]) -> Result<Address, StateReadRequestError> {
    if address_bytes.len() != ADDRESS_BYTES {
        return Err(StateReadRequestError::InvalidAddressLength {
            actual: address_bytes.len(),
        });
    }

    Ok(Address::from_slice(address_bytes))
}

fn parse_storage_slot(slot_bytes: &[u8]) -> Result<H256, StateReadRequestError> {
    if slot_bytes.len() != HASH_BYTES {
        return Err(StateReadRequestError::InvalidEspaceStorageSlotLength {
            actual: slot_bytes.len(),
        });
    }

    Ok(H256::from_slice(slot_bytes))
}

fn parse_code_hash(code_hash_bytes: &[u8]) -> Result<H256, StateReadRequestError> {
    if code_hash_bytes.len() != HASH_BYTES {
        return Err(StateReadRequestError::InvalidEspaceCodeHashLength {
            actual: code_hash_bytes.len(),
        });
    }

    Ok(H256::from_slice(code_hash_bytes))
}

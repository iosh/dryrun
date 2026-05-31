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
pub(crate) enum RpcStateKey {
    // eSpace account data is reconstructed from RPC account fields.
    EspaceAccount { address: Address },
    // eSpace storage is addressed by account + 32-byte slot.
    EspaceStorageSlot { address: Address, slot: H256 },
    // eSpace code is keyed by address + expected code hash.
    EspaceCode { address: Address, code_hash: H256 },
    // Native global params are fixed storage keys under system contracts.
    NativeGlobalParam(NativeGlobalParam),
}

impl RpcStateKey {
    // Resolve a raw Conflux storage key into the semantic key our RPC-backed
    // state layer will read.
    pub(crate) fn from_storage_key(
        storage_key: StorageKeyWithSpace<'_>,
    ) -> Result<Self, RpcStateKeyError> {
        match storage_key.space {
            Space::Ethereum => Self::from_espace_key(storage_key.key),
            Space::Native => Self::from_native_key(storage_key.key),
        }
    }

    fn from_espace_key(key: StorageKey<'_>) -> Result<Self, RpcStateKeyError> {
        match key {
            StorageKey::AccountKey(address_bytes) => Ok(Self::EspaceAccount {
                address: parse_address(address_bytes)?,
            }),
            StorageKey::StorageKey {
                address_bytes,
                storage_key,
            } => Ok(Self::EspaceStorageSlot {
                address: parse_address(address_bytes)?,
                slot: parse_storage_slot(storage_key)?,
            }),
            StorageKey::CodeKey {
                address_bytes,
                code_hash_bytes,
            } => Ok(Self::EspaceCode {
                address: parse_address(address_bytes)?,
                code_hash: parse_code_hash(code_hash_bytes)?,
            }),
            other => Err(RpcStateKeyError::UnsupportedEspaceKey {
                kind: StorageKeyKind::from_storage_key(other),
            }),
        }
    }

    fn from_native_key(key: StorageKey<'_>) -> Result<Self, RpcStateKeyError> {
        match key {
            StorageKey::StorageKey {
                address_bytes,
                storage_key,
            } => {
                let param = find_native_global_param(address_bytes, storage_key)
                    .ok_or(RpcStateKeyError::UnsupportedNativeGlobalParam)?;
                Ok(Self::NativeGlobalParam(param))
            }
            other => Err(RpcStateKeyError::UnsupportedNativeKey {
                kind: StorageKeyKind::from_storage_key(other),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativeGlobalParam {
    InterestRate,
    AccumulateInterestRate,
    TotalIssued,
    TotalStaking,
    TotalStorage,
    TotalEvmToken,
    UsedStoragePoints,
    ConvertedStoragePoints,
    TotalPosStaking,
    DistributablePoSInterest,
    LastDistributeBlock,
    PowBaseReward,
    TotalBurnt1559,
    BaseFeeProp,
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
pub(crate) enum RpcStateKeyError {
    #[error("unsupported eSpace storage key kind: {kind}")]
    UnsupportedEspaceKey { kind: StorageKeyKind },
    #[error("unsupported native storage key kind: {kind}")]
    UnsupportedNativeKey { kind: StorageKeyKind },
    #[error("unsupported native global parameter storage key")]
    UnsupportedNativeGlobalParam,
    #[error("invalid address length: expected {ADDRESS_BYTES} bytes, got {actual}")]
    InvalidAddressLength { actual: usize },
    #[error("invalid eSpace storage slot length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidEspaceStorageSlotLength { actual: usize },
    #[error("invalid eSpace code hash length: expected {HASH_BYTES} bytes, got {actual}")]
    InvalidEspaceCodeHashLength { actual: usize },
}

fn find_native_global_param(address_bytes: &[u8], storage_key: &[u8]) -> Option<NativeGlobalParam> {
    if is_native_global_param::<InterestRate>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::InterestRate);
    }
    if is_native_global_param::<AccumulateInterestRate>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::AccumulateInterestRate);
    }

    if is_native_global_param::<TotalIssued>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalIssued);
    }
    if is_native_global_param::<TotalStaking>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalStaking);
    }
    if is_native_global_param::<TotalStorage>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalStorage);
    }
    if is_native_global_param::<TotalEvmToken>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalEvmToken);
    }
    if is_native_global_param::<UsedStoragePoints>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::UsedStoragePoints);
    }
    if is_native_global_param::<ConvertedStoragePoints>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::ConvertedStoragePoints);
    }
    if is_native_global_param::<TotalPosStaking>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalPosStaking);
    }
    if is_native_global_param::<DistributablePoSInterest>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::DistributablePoSInterest);
    }
    if is_native_global_param::<LastDistributeBlock>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::LastDistributeBlock);
    }
    if is_native_global_param::<PowBaseReward>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::PowBaseReward);
    }
    if is_native_global_param::<TotalBurnt1559>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::TotalBurnt1559);
    }
    if is_native_global_param::<BaseFeeProp>(address_bytes, storage_key) {
        return Some(NativeGlobalParam::BaseFeeProp);
    }

    None
}

fn is_native_global_param<T: GlobalParamKey>(address_bytes: &[u8], storage_key: &[u8]) -> bool {
    address_bytes == T::ADDRESS.as_bytes() && storage_key == T::KEY
}

fn parse_address(address_bytes: &[u8]) -> Result<Address, RpcStateKeyError> {
    if address_bytes.len() != ADDRESS_BYTES {
        return Err(RpcStateKeyError::InvalidAddressLength {
            actual: address_bytes.len(),
        });
    }

    Ok(Address::from_slice(address_bytes))
}

fn parse_storage_slot(slot_bytes: &[u8]) -> Result<H256, RpcStateKeyError> {
    if slot_bytes.len() != HASH_BYTES {
        return Err(RpcStateKeyError::InvalidEspaceStorageSlotLength {
            actual: slot_bytes.len(),
        });
    }

    Ok(H256::from_slice(slot_bytes))
}

fn parse_code_hash(code_hash_bytes: &[u8]) -> Result<H256, RpcStateKeyError> {
    if code_hash_bytes.len() != HASH_BYTES {
        return Err(RpcStateKeyError::InvalidEspaceCodeHashLength {
            actual: code_hash_bytes.len(),
        });
    }

    Ok(H256::from_slice(code_hash_bytes))
}

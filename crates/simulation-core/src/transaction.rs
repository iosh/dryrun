pub use alloy_consensus::TxType;
pub use alloy_eips::eip7702::{Authorization, SignedAuthorization};
use alloy_primitives::{Address, B256, Bytes, U256};
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "serde")]
use serde_with::{As, TryFromInto};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct TransactionCommon<A = Address, N = u64, C = u64> {
    pub from: A,
    pub to: Option<A>,
    #[cfg_attr(
        feature = "serde",
        serde(
            with = "As::<TryFromInto<U256>>",
            bound(serialize = "N: Copy + TryInto<U256, Error: std::fmt::Display>")
        )
    )]
    pub nonce: N,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "gas", with = "As::<TryFromInto<U256>>")
    )]
    pub gas_limit: N,
    pub value: U256,
    #[cfg_attr(feature = "serde", serde(rename = "data"))]
    pub input: Bytes,
    #[cfg_attr(
        feature = "serde",
        serde(
            with = "As::<TryFromInto<U256>>",
            bound(serialize = "C: Copy + TryInto<U256, Error: std::fmt::Display>")
        )
    )]
    pub chain_id: C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct AccessListItem<A = Address> {
    pub address: A,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DynamicFees {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", rename_all_fields = "camelCase")
)]
pub enum TypedTransaction {
    #[cfg_attr(feature = "serde", serde(rename = "0x0"))]
    Legacy {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: TransactionCommon,
        gas_price: U256,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x1"))]
    Eip2930 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: TransactionCommon,
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x2"))]
    Eip1559 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: TransactionCommon,
        #[cfg_attr(feature = "serde", serde(flatten))]
        fees: DynamicFees,
        access_list: Vec<AccessListItem>,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x3"))]
    Eip4844 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: TransactionCommon,
        #[cfg_attr(feature = "serde", serde(flatten))]
        fees: DynamicFees,
        max_fee_per_blob_gas: U256,
        access_list: Vec<AccessListItem>,
        blob_versioned_hashes: Vec<B256>,
    },
    #[cfg_attr(feature = "serde", serde(rename = "0x4"))]
    Eip7702 {
        #[cfg_attr(feature = "serde", serde(flatten))]
        common: TransactionCommon,
        #[cfg_attr(feature = "serde", serde(flatten))]
        fees: DynamicFees,
        access_list: Vec<AccessListItem>,
        #[cfg_attr(
            feature = "serde",
            serde(serialize_with = "crate::codec::serialize_authorizations")
        )]
        authorization_list: Vec<SignedAuthorization>,
    },
}

impl TypedTransaction {
    pub fn common(&self) -> &TransactionCommon {
        match self {
            Self::Legacy { common, .. }
            | Self::Eip2930 { common, .. }
            | Self::Eip1559 { common, .. }
            | Self::Eip4844 { common, .. }
            | Self::Eip7702 { common, .. } => common,
        }
    }

    pub fn common_mut(&mut self) -> &mut TransactionCommon {
        match self {
            Self::Legacy { common, .. }
            | Self::Eip2930 { common, .. }
            | Self::Eip1559 { common, .. }
            | Self::Eip4844 { common, .. }
            | Self::Eip7702 { common, .. } => common,
        }
    }

    pub fn validate_fields(&self) -> Result<(), TransactionInputError> {
        match self {
            Self::Eip4844 {
                common,
                blob_versioned_hashes,
                ..
            } => {
                require_recipient(common.to, TxType::Eip4844)?;
                require_non_empty_list(
                    blob_versioned_hashes,
                    TxType::Eip4844,
                    "blobVersionedHashes",
                )
            }
            Self::Eip7702 {
                common,
                authorization_list,
                ..
            } => {
                require_recipient(common.to, TxType::Eip7702)?;
                require_non_empty_list(authorization_list, TxType::Eip7702, "authorizationList")
            }
            _ => Ok(()),
        }
    }

    pub fn transaction_type(&self) -> TxType {
        match self {
            Self::Legacy { .. } => TxType::Legacy,
            Self::Eip2930 { .. } => TxType::Eip2930,
            Self::Eip1559 { .. } => TxType::Eip1559,
            Self::Eip4844 { .. } => TxType::Eip4844,
            Self::Eip7702 { .. } => TxType::Eip7702,
        }
    }

    pub fn gas_price_cap(&self) -> U256 {
        match self {
            Self::Legacy { gas_price, .. } | Self::Eip2930 { gas_price, .. } => *gas_price,
            Self::Eip1559 { fees, .. }
            | Self::Eip4844 { fees, .. }
            | Self::Eip7702 { fees, .. } => fees.max_fee_per_gas,
        }
    }

    pub fn dynamic_fees(&self) -> Option<DynamicFees> {
        match self {
            Self::Eip1559 { fees, .. }
            | Self::Eip4844 { fees, .. }
            | Self::Eip7702 { fees, .. } => Some(*fees),
            _ => None,
        }
    }

    pub fn access_list(&self) -> &[AccessListItem] {
        match self {
            Self::Legacy { .. } => &[],
            Self::Eip2930 { access_list, .. }
            | Self::Eip1559 { access_list, .. }
            | Self::Eip4844 { access_list, .. }
            | Self::Eip7702 { access_list, .. } => access_list,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TransactionInputError {
    #[error("{transaction_type} transactions do not accept transaction.{field}")]
    IncompatibleField {
        transaction_type: TxType,
        field: &'static str,
    },
    #[error("{transaction_type} transactions require transaction.{field}")]
    MissingField {
        transaction_type: TxType,
        field: &'static str,
    },
    #[error("transaction.{field} exceeds the supported maximum {maximum:#x}: {value:#x}")]
    OutOfRange {
        field: &'static str,
        value: U256,
        maximum: U256,
    },
    #[error("transaction type {transaction_type} is not supported by this execution backend")]
    UnsupportedType { transaction_type: TxType },
}

fn require_recipient(
    to: Option<Address>,
    transaction_type: TxType,
) -> Result<(), TransactionInputError> {
    if to.is_none() {
        return Err(TransactionInputError::MissingField {
            transaction_type,
            field: "to",
        });
    }
    Ok(())
}

fn require_non_empty_list<T>(
    items: &[T],
    transaction_type: TxType,
    field: &'static str,
) -> Result<(), TransactionInputError> {
    if items.is_empty() {
        return Err(TransactionInputError::MissingField {
            transaction_type,
            field,
        });
    }
    Ok(())
}

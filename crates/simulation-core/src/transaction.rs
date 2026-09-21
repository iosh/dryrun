use alloy_primitives::{Address, B256, Bytes, U256};
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "serde")]
use serde_with::{As, TryFromInto};
use thiserror::Error;

pub use alloy_consensus::TxType;
pub use alloy_eips::eip7702::{Authorization, SignedAuthorization};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "status", content = "fields", rename_all = "camelCase")
)]
pub enum TransactionInput<C = TypedTransaction, P = TransactionRequest> {
    Complete(C),
    Partial(P),
}

impl<C, P> TransactionInput<C, P> {
    pub fn as_ref(&self) -> TransactionInput<&C, &P> {
        match self {
            Self::Complete(transaction) => TransactionInput::Complete(transaction),
            Self::Partial(transaction) => TransactionInput::Partial(transaction),
        }
    }
}

pub type TransactionRef<'a> = TransactionInput<&'a TypedTransaction, &'a TransactionRequest>;

impl TransactionRef<'_> {
    pub fn chain_id(&self) -> Option<u64> {
        match self {
            Self::Complete(tx) => Some(tx.common().chain_id),
            Self::Partial(tx) => tx.common.chain_id,
        }
    }

    pub fn gas_limit(&self) -> Option<u64> {
        match self {
            Self::Complete(tx) => Some(tx.common().gas_limit),
            Self::Partial(tx) => tx.common.gas_limit,
        }
    }

    pub fn to(&self) -> Option<Address> {
        match self {
            Self::Complete(tx) => tx.common().to,
            Self::Partial(tx) => tx.common.to,
        }
    }

    pub fn input(&self) -> &[u8] {
        match self {
            Self::Complete(tx) => &tx.common().input,
            Self::Partial(tx) => tx.common.input.as_ref().map_or(&[], |input| input.as_ref()),
        }
    }

    pub fn gas_price_cap(&self) -> Option<U256> {
        match self {
            Self::Complete(tx) => Some(tx.gas_price_cap()),
            Self::Partial(tx) => tx.fees.gas_price.or(tx.fees.max_fee_per_gas),
        }
    }

    pub fn priority_fee(&self) -> Option<U256> {
        match self {
            Self::Complete(tx) => tx.dynamic_fees().map(|fees| fees.max_priority_fee_per_gas),
            Self::Partial(tx) => tx.fees.max_priority_fee_per_gas,
        }
    }
}

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
pub struct PartialTransactionCommon<A = Address, N = u64, C = u64> {
    pub from: A,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub to: Option<A>,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            with = "As::<Option<TryFromInto<U256>>>",
            bound(serialize = "N: Copy + TryInto<U256, Error: std::fmt::Display>")
        )
    )]
    pub nonce: Option<N>,
    #[cfg_attr(
        feature = "serde",
        serde(
            rename = "gas",
            skip_serializing_if = "Option::is_none",
            with = "As::<Option<TryFromInto<U256>>>"
        )
    )]
    pub gas_limit: Option<N>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub value: Option<U256>,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "data", skip_serializing_if = "Option::is_none")
    )]
    pub input: Option<Bytes>,
    #[cfg_attr(
        feature = "serde",
        serde(
            skip_serializing_if = "Option::is_none",
            with = "As::<Option<TryFromInto<U256>>>",
            bound(serialize = "C: Copy + TryInto<U256, Error: std::fmt::Display>")
        )
    )]
    pub chain_id: Option<C>,
}

impl<A, N, C> PartialTransactionCommon<A, N, C> {
    pub fn new(from: A) -> Self {
        Self {
            from,
            to: None,
            nonce: None,
            gas_limit: None,
            value: None,
            input: None,
            chain_id: None,
        }
    }
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FeeInput {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub gas_price: Option<U256>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_fee_per_gas: Option<U256>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_priority_fee_per_gas: Option<U256>,
}

impl FeeInput {
    pub fn has_dynamic_fees(&self) -> bool {
        self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some()
    }

    pub fn validate_fee_conflicts(&self) -> Result<(), TransactionInputError> {
        if self.gas_price.is_some() && self.has_dynamic_fees() {
            return Err(TransactionInputError::ConflictingFields {
                first: "gasPrice",
                second: "maxFeePerGas / maxPriorityFeePerGas",
            });
        }
        Ok(())
    }
}

/// A transaction with all execution fields supplied, before chain-specific checks.
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

    pub fn check_type_requirements(&self) -> Result<(), TransactionInputError> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct TransactionRequest {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub common: PartialTransactionCommon,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub fees: FeeInput,
    #[cfg_attr(
        feature = "serde",
        serde(
            rename = "type",
            skip_serializing_if = "Option::is_none",
            serialize_with = "crate::codec::transaction::serialize_transaction_type"
        )
    )]
    pub transaction_type: Option<TxType>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_fee_per_blob_gas: Option<U256>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub access_list: Option<Vec<AccessListItem>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub blob_versioned_hashes: Option<Vec<B256>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub authorization_list: Option<Vec<SignedAuthorization>>,
}

impl TransactionRequest {
    pub fn new(from: Address) -> Self {
        Self {
            common: PartialTransactionCommon::new(from),
            fees: FeeInput::default(),
            transaction_type: None,
            max_fee_per_blob_gas: None,
            access_list: None,
            blob_versioned_hashes: None,
            authorization_list: None,
        }
    }

    pub fn transaction_type(&self, default: TxType) -> Result<TxType, TransactionInputError> {
        let inferred = if self.authorization_list.is_some() {
            TxType::Eip7702
        } else if self.blob_versioned_hashes.is_some() || self.max_fee_per_blob_gas.is_some() {
            TxType::Eip4844
        } else if self.fees.has_dynamic_fees() {
            TxType::Eip1559
        } else if self.fees.gas_price.is_some() {
            if self.access_list.is_some() {
                TxType::Eip2930
            } else {
                TxType::Legacy
            }
        } else if self.access_list.is_some() && default == TxType::Legacy {
            TxType::Eip2930
        } else {
            default
        };
        let transaction_type = self.transaction_type.unwrap_or(inferred);
        self.check_type_requirements(transaction_type)?;
        Ok(transaction_type)
    }

    fn check_type_requirements(
        &self,
        transaction_type: TxType,
    ) -> Result<(), TransactionInputError> {
        self.fees.validate_fee_conflicts()?;
        let dynamic = matches!(
            transaction_type,
            TxType::Eip1559 | TxType::Eip4844 | TxType::Eip7702
        );
        let fields = [
            ("gasPrice", self.fees.gas_price.is_some(), !dynamic),
            ("maxFeePerGas", self.fees.max_fee_per_gas.is_some(), dynamic),
            (
                "maxPriorityFeePerGas",
                self.fees.max_priority_fee_per_gas.is_some(),
                dynamic,
            ),
            (
                "accessList",
                self.access_list.is_some(),
                transaction_type != TxType::Legacy,
            ),
            (
                "maxFeePerBlobGas",
                self.max_fee_per_blob_gas.is_some(),
                transaction_type == TxType::Eip4844,
            ),
            (
                "blobVersionedHashes",
                self.blob_versioned_hashes.is_some(),
                transaction_type == TxType::Eip4844,
            ),
            (
                "authorizationList",
                self.authorization_list.is_some(),
                transaction_type == TxType::Eip7702,
            ),
        ];
        for (field, present, allowed) in fields {
            if present && !allowed {
                return Err(TransactionInputError::IncompatibleField {
                    transaction_type,
                    field,
                });
            }
        }
        match transaction_type {
            TxType::Eip4844 => {
                require_recipient(self.common.to, transaction_type)?;
                require_non_empty_list(
                    self.blob_versioned_hashes.as_deref().unwrap_or_default(),
                    transaction_type,
                    "blobVersionedHashes",
                )
            }
            TxType::Eip7702 => {
                require_recipient(self.common.to, transaction_type)?;
                require_non_empty_list(
                    self.authorization_list.as_deref().unwrap_or_default(),
                    transaction_type,
                    "authorizationList",
                )
            }
            _ => Ok(()),
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
    #[error("transaction.{first} conflicts with transaction.{second}")]
    ConflictingFields {
        first: &'static str,
        second: &'static str,
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

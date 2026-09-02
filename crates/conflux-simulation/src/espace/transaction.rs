use alloy::primitives::{Address, B256, Bytes, U256};
use thiserror::Error;

pub use alloy::consensus::TxType;
pub use alloy::eips::{
    eip2930::AccessListItem,
    eip7702::{Authorization, SignedAuthorization},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceTransactionInput {
    Complete(EspaceCompleteTransaction),
    Partial(EspacePartialTransaction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceTransactionCommon {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub input: Bytes,
    pub chain_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceCompleteTransaction {
    Legacy {
        common: EspaceTransactionCommon,
        gas_price: U256,
    },
    Eip2930 {
        common: EspaceTransactionCommon,
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        common: EspaceTransactionCommon,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
    Eip7702 {
        common: EspaceTransactionCommon,
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
        authorization_list: Vec<SignedAuthorization>,
    },
}

impl EspaceCompleteTransaction {
    pub fn common(&self) -> &EspaceTransactionCommon {
        match self {
            Self::Legacy { common, .. }
            | Self::Eip2930 { common, .. }
            | Self::Eip1559 { common, .. }
            | Self::Eip7702 { common, .. } => common,
        }
    }

    pub(crate) fn common_mut(&mut self) -> &mut EspaceTransactionCommon {
        match self {
            Self::Legacy { common, .. }
            | Self::Eip2930 { common, .. }
            | Self::Eip1559 { common, .. }
            | Self::Eip7702 { common, .. } => common,
        }
    }

    pub fn transaction_type(&self) -> TxType {
        match self {
            Self::Legacy { .. } => TxType::Legacy,
            Self::Eip2930 { .. } => TxType::Eip2930,
            Self::Eip1559 { .. } => TxType::Eip1559,
            Self::Eip7702 { .. } => TxType::Eip7702,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), EspaceTransactionInputError> {
        match self {
            Self::Eip7702 {
                common,
                authorization_list,
                ..
            } => validate_eip7702_requirements(common.to, authorization_list),
            Self::Legacy { .. } | Self::Eip2930 { .. } | Self::Eip1559 { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspacePartialTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub chain_id: Option<u64>,
    pub transaction_type: Option<TxType>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
    pub max_fee_per_blob_gas: Option<U256>,
    pub access_list: Option<Vec<AccessListItem>>,
    pub blob_versioned_hashes: Option<Vec<B256>>,
    pub authorization_list: Option<Vec<SignedAuthorization>>,
}

impl EspacePartialTransaction {
    pub(crate) fn preferred_type(&self, has_base_fee: bool) -> TxType {
        if self.authorization_list.is_some() {
            TxType::Eip7702
        } else if self.blob_versioned_hashes.is_some() {
            TxType::Eip4844
        } else if self.access_list.is_some() && self.gas_price.is_some() {
            TxType::Eip2930
        } else if self.gas_price.is_some() {
            TxType::Legacy
        } else if self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some() {
            TxType::Eip1559
        } else if has_base_fee {
            TxType::Eip1559
        } else if self.access_list.is_some() {
            TxType::Eip2930
        } else {
            TxType::Legacy
        }
    }

    pub(crate) fn validate(
        &self,
        transaction_type: TxType,
    ) -> Result<(), EspaceTransactionInputError> {
        match transaction_type {
            TxType::Legacy => {
                reject_present(self.access_list.is_some(), transaction_type, "accessList")?;
                reject_present(
                    self.max_fee_per_gas.is_some(),
                    transaction_type,
                    "maxFeePerGas",
                )?;
                reject_present(
                    self.max_priority_fee_per_gas.is_some(),
                    transaction_type,
                    "maxPriorityFeePerGas",
                )?;
                reject_present(
                    self.max_fee_per_blob_gas.is_some(),
                    transaction_type,
                    "maxFeePerBlobGas",
                )?;
                reject_present(
                    self.blob_versioned_hashes.is_some(),
                    transaction_type,
                    "blobVersionedHashes",
                )?;
                reject_present(
                    self.authorization_list.is_some(),
                    transaction_type,
                    "authorizationList",
                )
            }
            TxType::Eip2930 => {
                reject_present(
                    self.max_fee_per_gas.is_some(),
                    transaction_type,
                    "maxFeePerGas",
                )?;
                reject_present(
                    self.max_priority_fee_per_gas.is_some(),
                    transaction_type,
                    "maxPriorityFeePerGas",
                )?;
                reject_present(
                    self.max_fee_per_blob_gas.is_some(),
                    transaction_type,
                    "maxFeePerBlobGas",
                )?;
                reject_present(
                    self.blob_versioned_hashes.is_some(),
                    transaction_type,
                    "blobVersionedHashes",
                )?;
                reject_present(
                    self.authorization_list.is_some(),
                    transaction_type,
                    "authorizationList",
                )
            }
            TxType::Eip1559 => {
                reject_present(self.gas_price.is_some(), transaction_type, "gasPrice")?;
                reject_present(
                    self.max_fee_per_blob_gas.is_some(),
                    transaction_type,
                    "maxFeePerBlobGas",
                )?;
                reject_present(
                    self.blob_versioned_hashes.is_some(),
                    transaction_type,
                    "blobVersionedHashes",
                )?;
                reject_present(
                    self.authorization_list.is_some(),
                    transaction_type,
                    "authorizationList",
                )
            }
            TxType::Eip4844 => Ok(()),
            TxType::Eip7702 => {
                reject_present(self.gas_price.is_some(), transaction_type, "gasPrice")?;
                reject_present(
                    self.max_fee_per_blob_gas.is_some(),
                    transaction_type,
                    "maxFeePerBlobGas",
                )?;
                reject_present(
                    self.blob_versioned_hashes.is_some(),
                    transaction_type,
                    "blobVersionedHashes",
                )?;
                validate_eip7702_requirements(
                    self.to,
                    self.authorization_list.as_deref().unwrap_or_default(),
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EspaceTransactionInputError {
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
}

fn reject_present(
    present: bool,
    transaction_type: TxType,
    field: &'static str,
) -> Result<(), EspaceTransactionInputError> {
    if present {
        Err(EspaceTransactionInputError::IncompatibleField {
            transaction_type,
            field,
        })
    } else {
        Ok(())
    }
}

fn validate_eip7702_requirements(
    to: Option<Address>,
    authorization_list: &[SignedAuthorization],
) -> Result<(), EspaceTransactionInputError> {
    if to.is_none() {
        return Err(EspaceTransactionInputError::MissingField {
            transaction_type: TxType::Eip7702,
            field: "to",
        });
    }
    if authorization_list.is_empty() {
        return Err(EspaceTransactionInputError::MissingField {
            transaction_type: TxType::Eip7702,
            field: "authorizationList",
        });
    }

    Ok(())
}

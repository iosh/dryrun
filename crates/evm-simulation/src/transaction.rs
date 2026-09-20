use alloy::primitives::{Address, B256, Bytes, U256};

pub use simulation_core::transaction::{
    AccessListItem, Authorization, DynamicFees, SignedAuthorization, TransactionCommon,
    TransactionInputError, TxType, TypedTransaction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionInput {
    Complete(TypedTransaction),
    Partial(PartialTransaction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub chain_id: Option<u64>,
    pub transaction_type: Option<TxType>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub max_fee_per_blob_gas: Option<u128>,
    pub access_list: Option<Vec<AccessListItem>>,
    pub blob_versioned_hashes: Option<Vec<B256>>,
    pub authorization_list: Option<Vec<SignedAuthorization>>,
}

impl PartialTransaction {
    pub(crate) fn preferred_type(&self) -> TxType {
        if self.authorization_list.is_some() {
            TxType::Eip7702
        } else if self.blob_versioned_hashes.is_some() {
            TxType::Eip4844
        } else if self.access_list.is_some() && self.gas_price.is_some() {
            TxType::Eip2930
        } else if self.gas_price.is_some() {
            TxType::Legacy
        } else {
            TxType::Eip1559
        }
    }

    pub(crate) fn validate(&self, transaction_type: TxType) -> Result<(), TransactionInputError> {
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
            TxType::Eip4844 => {
                reject_present(self.gas_price.is_some(), transaction_type, "gasPrice")?;
                reject_present(
                    self.authorization_list.is_some(),
                    transaction_type,
                    "authorizationList",
                )?;
                validate_eip4844_requirements(
                    self.to,
                    self.blob_versioned_hashes.as_deref().unwrap_or_default(),
                )
            }
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

fn reject_present(
    present: bool,
    transaction_type: TxType,
    field: &'static str,
) -> Result<(), TransactionInputError> {
    if present {
        Err(TransactionInputError::IncompatibleField {
            transaction_type,
            field,
        })
    } else {
        Ok(())
    }
}

fn validate_eip4844_requirements(
    to: Option<Address>,
    blob_versioned_hashes: &[B256],
) -> Result<(), TransactionInputError> {
    if to.is_none() {
        return Err(TransactionInputError::MissingField {
            transaction_type: TxType::Eip4844,
            field: "to",
        });
    }
    if blob_versioned_hashes.is_empty() {
        return Err(TransactionInputError::MissingField {
            transaction_type: TxType::Eip4844,
            field: "blobVersionedHashes",
        });
    }

    Ok(())
}

fn validate_eip7702_requirements(
    to: Option<Address>,
    authorization_list: &[SignedAuthorization],
) -> Result<(), TransactionInputError> {
    if to.is_none() {
        return Err(TransactionInputError::MissingField {
            transaction_type: TxType::Eip7702,
            field: "to",
        });
    }
    if authorization_list.is_empty() {
        return Err(TransactionInputError::MissingField {
            transaction_type: TxType::Eip7702,
            field: "authorizationList",
        });
    }

    Ok(())
}

pub(crate) fn checked_fee(
    field: &'static str,
    value: U256,
) -> Result<u128, crate::TransactionInputError> {
    u128::try_from(value).map_err(|_| crate::TransactionInputError::OutOfRange {
        field,
        value,
        maximum: U256::from(u128::MAX),
    })
}

use alloy::primitives::{Address, B256, Bytes, U256};
use thiserror::Error;

pub use alloy::eips::{
    eip2930::AccessListItem,
    eip7702::{Authorization, SignedAuthorization},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionInput {
    Complete(CompleteTransaction),
    Partial(PartialTransaction),
}

impl TransactionInput {
    pub(crate) fn validate_requirements(&self) -> Result<(), TransactionInputError> {
        match self {
            Self::Complete(transaction) => transaction.validate_requirements(),
            Self::Partial(transaction) => transaction.validate_requirements(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub input: Bytes,
    pub chain_id: u64,
    pub variant: CompleteTransactionVariant,
}

impl CompleteTransaction {
    fn validate_requirements(&self) -> Result<(), TransactionInputError> {
        match &self.variant {
            CompleteTransactionVariant::Eip4844 {
                blob_versioned_hashes,
                ..
            } => validate_eip4844_requirements(self.to, blob_versioned_hashes),
            CompleteTransactionVariant::Eip7702 {
                authorization_list, ..
            } => validate_eip7702_requirements(self.to, authorization_list),
            CompleteTransactionVariant::Legacy { .. }
            | CompleteTransactionVariant::Eip2930 { .. }
            | CompleteTransactionVariant::Eip1559 { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompleteTransactionVariant {
    Legacy {
        gas_price: u128,
    },
    Eip2930 {
        gas_price: u128,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        access_list: Vec<AccessListItem>,
    },
    Eip4844 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        max_fee_per_blob_gas: u128,
        access_list: Vec<AccessListItem>,
        blob_versioned_hashes: Vec<B256>,
    },
    Eip7702 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        access_list: Vec<AccessListItem>,
        authorization_list: Vec<SignedAuthorization>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub variant: PartialTransactionVariant,
}

impl PartialTransaction {
    fn validate_requirements(&self) -> Result<(), TransactionInputError> {
        match &self.variant {
            PartialTransactionVariant::Eip4844 {
                blob_versioned_hashes,
                ..
            } => validate_eip4844_requirements(self.to, blob_versioned_hashes),
            PartialTransactionVariant::Eip7702 {
                authorization_list, ..
            } => validate_eip7702_requirements(self.to, authorization_list),
            PartialTransactionVariant::Legacy { .. }
            | PartialTransactionVariant::Eip2930 { .. }
            | PartialTransactionVariant::Eip1559 { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartialTransactionVariant {
    Legacy {
        gas_price: Option<u128>,
    },
    Eip2930 {
        gas_price: Option<u128>,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
        access_list: Vec<AccessListItem>,
    },
    Eip4844 {
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
        max_fee_per_blob_gas: Option<u128>,
        access_list: Vec<AccessListItem>,
        blob_versioned_hashes: Vec<B256>,
    },
    Eip7702 {
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
        access_list: Vec<AccessListItem>,
        authorization_list: Vec<SignedAuthorization>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TransactionInputError {
    #[error("EIP-4844 transactions require a call destination")]
    Eip4844CallDestinationRequired,

    #[error("EIP-4844 transactions require at least one blob versioned hash")]
    Eip4844BlobVersionedHashesRequired,

    #[error("EIP-7702 transactions require a call destination")]
    Eip7702CallDestinationRequired,

    #[error("EIP-7702 transactions require at least one signed authorization")]
    Eip7702AuthorizationListRequired,
}

fn validate_eip4844_requirements(
    to: Option<Address>,
    blob_versioned_hashes: &[B256],
) -> Result<(), TransactionInputError> {
    if to.is_none() {
        return Err(TransactionInputError::Eip4844CallDestinationRequired);
    }
    if blob_versioned_hashes.is_empty() {
        return Err(TransactionInputError::Eip4844BlobVersionedHashesRequired);
    }

    Ok(())
}

fn validate_eip7702_requirements(
    to: Option<Address>,
    authorization_list: &[SignedAuthorization],
) -> Result<(), TransactionInputError> {
    if to.is_none() {
        return Err(TransactionInputError::Eip7702CallDestinationRequired);
    }
    if authorization_list.is_empty() {
        return Err(TransactionInputError::Eip7702AuthorizationListRequired);
    }

    Ok(())
}

use alloy::primitives::{Address, Bytes, U256};
use thiserror::Error;

pub use alloy::eips::{
    eip2930::AccessListItem,
    eip7702::{Authorization, SignedAuthorization},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceTransactionInput {
    Complete(EspaceCompleteTransaction),
    Partial(EspacePartialTransaction),
}

impl EspaceTransactionInput {
    pub(crate) fn validate_requirements(&self) -> Result<(), EspaceTransactionInputError> {
        match self {
            Self::Complete(transaction) => transaction.validate_requirements(),
            Self::Partial(transaction) => transaction.validate_requirements(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceCompleteTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub input: Bytes,
    pub chain_id: u64,
    pub variant: EspaceCompleteTransactionVariant,
}

impl EspaceCompleteTransaction {
    fn validate_requirements(&self) -> Result<(), EspaceTransactionInputError> {
        match &self.variant {
            EspaceCompleteTransactionVariant::Eip7702 {
                authorization_list, ..
            } => validate_eip7702_requirements(self.to, authorization_list),
            EspaceCompleteTransactionVariant::Legacy { .. }
            | EspaceCompleteTransactionVariant::Eip2930 { .. }
            | EspaceCompleteTransactionVariant::Eip1559 { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceCompleteTransactionVariant {
    Legacy {
        gas_price: U256,
    },
    Eip2930 {
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
    Eip7702 {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
        authorization_list: Vec<SignedAuthorization>,
    },
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
    pub variant: EspacePartialTransactionVariant,
}

impl EspacePartialTransaction {
    fn validate_requirements(&self) -> Result<(), EspaceTransactionInputError> {
        match &self.variant {
            EspacePartialTransactionVariant::Eip7702 {
                authorization_list, ..
            } => validate_eip7702_requirements(self.to, authorization_list),
            EspacePartialTransactionVariant::Legacy { .. }
            | EspacePartialTransactionVariant::Eip2930 { .. }
            | EspacePartialTransactionVariant::Eip1559 { .. } => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspacePartialTransactionVariant {
    Legacy {
        gas_price: Option<U256>,
    },
    Eip2930 {
        gas_price: Option<U256>,
        access_list: Vec<AccessListItem>,
    },
    Eip1559 {
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        access_list: Vec<AccessListItem>,
    },
    Eip7702 {
        max_fee_per_gas: Option<U256>,
        max_priority_fee_per_gas: Option<U256>,
        access_list: Vec<AccessListItem>,
        authorization_list: Vec<SignedAuthorization>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum EspaceTransactionInputError {
    #[error("EIP-7702 transactions require a call destination")]
    Eip7702CallDestinationRequired,

    #[error("EIP-7702 transactions require at least one signed authorization")]
    Eip7702AuthorizationListRequired,
}

fn validate_eip7702_requirements(
    to: Option<Address>,
    authorization_list: &[SignedAuthorization],
) -> Result<(), EspaceTransactionInputError> {
    if to.is_none() {
        return Err(EspaceTransactionInputError::Eip7702CallDestinationRequired);
    }
    if authorization_list.is_empty() {
        return Err(EspaceTransactionInputError::Eip7702AuthorizationListRequired);
    }

    Ok(())
}

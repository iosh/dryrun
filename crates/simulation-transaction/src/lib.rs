use std::fmt;

pub use alloy_eips::eip2930::AccessListItem;
use alloy_primitives::{Address, Bytes, U256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRequest {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: Option<u64>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub chain_id: u64,
    pub variant: TransactionVariantRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub gas_limit: u64,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u64,
    pub variant: TransactionVariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Legacy,
    AccessList,
    DynamicFee,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Legacy => "legacy",
            Self::AccessList => "access-list",
            Self::DynamicFee => "dynamic-fee",
        })
    }
}

impl TransactionType {
    pub fn infer(explicit: Option<Self>, has_access_list: bool, has_dynamic_fee: bool) -> Self {
        match explicit {
            Some(transaction_type) => transaction_type,
            None if has_dynamic_fee => Self::DynamicFee,
            None if has_access_list => Self::AccessList,
            None => Self::Legacy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionVariantRequest {
    Legacy {
        gas_price: Option<u128>,
    },
    AccessList {
        gas_price: Option<u128>,
        access_list: Vec<AccessListItem>,
    },
    DynamicFee {
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
        access_list: Vec<AccessListItem>,
    },
}

impl TransactionVariantRequest {
    pub fn try_new(
        transaction_type: TransactionType,
        access_list: Option<Vec<AccessListItem>>,
        gas_price: Option<u128>,
        max_fee_per_gas: Option<u128>,
        max_priority_fee_per_gas: Option<u128>,
    ) -> Result<Self, TransactionVariantError> {
        let has_dynamic_fee = max_fee_per_gas.is_some() || max_priority_fee_per_gas.is_some();

        match transaction_type {
            TransactionType::Legacy => {
                if access_list.as_ref().is_some_and(|items| !items.is_empty()) {
                    return Err(TransactionVariantError::AccessListNotAllowed { transaction_type });
                }

                if has_dynamic_fee {
                    return Err(TransactionVariantError::DynamicFeeNotAllowed { transaction_type });
                }

                Ok(Self::Legacy { gas_price })
            }
            TransactionType::AccessList => {
                if has_dynamic_fee {
                    return Err(TransactionVariantError::DynamicFeeNotAllowed { transaction_type });
                }

                Ok(Self::AccessList {
                    gas_price,
                    access_list: access_list.unwrap_or_default(),
                })
            }
            TransactionType::DynamicFee => {
                if gas_price.is_some() {
                    return Err(TransactionVariantError::GasPriceNotAllowed { transaction_type });
                }

                Ok(Self::DynamicFee {
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    access_list: access_list.unwrap_or_default(),
                })
            }
        }
    }

    pub fn transaction_type(&self) -> TransactionType {
        match self {
            Self::Legacy { .. } => TransactionType::Legacy,
            Self::AccessList { .. } => TransactionType::AccessList,
            Self::DynamicFee { .. } => TransactionType::DynamicFee,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionVariant {
    Legacy {
        gas_price: u128,
    },
    AccessList {
        gas_price: u128,
        access_list: Vec<AccessListItem>,
    },
    DynamicFee {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionVariantError {
    #[error("{transaction_type} transactions cannot include an access list")]
    AccessListNotAllowed { transaction_type: TransactionType },

    #[error("{transaction_type} transactions cannot include dynamic fee fields")]
    DynamicFeeNotAllowed { transaction_type: TransactionType },

    #[error("{transaction_type} transactions cannot include a gas price")]
    GasPriceNotAllowed { transaction_type: TransactionType },
}

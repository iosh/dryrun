use std::fmt;

use alloy_primitives::{Address, B256, Bytes, U256};
use thiserror::Error;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRequest {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<U256>,
    pub gas_limit: Option<U256>,
    pub value: Option<U256>,
    pub input: Option<Bytes>,
    pub chain_id: Option<U256>,
    pub transaction_type: Option<TransactionType>,
    pub access_list: Option<Vec<AccessListItem>>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
}

impl TransactionRequest {
    pub fn resolved_type(&self) -> TransactionType {
        match self.transaction_type {
            Some(transaction_type) => transaction_type,
            None if self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some() => {
                TransactionType::DynamicFee
            }
            None if self.access_list.is_some() => TransactionType::AccessList,
            None => TransactionType::Legacy,
        }
    }

    pub fn validate_shape(&self) -> Result<(), TransactionRequestError> {
        let transaction_type = self.resolved_type();
        let has_dynamic_fee =
            self.max_fee_per_gas.is_some() || self.max_priority_fee_per_gas.is_some();

        match transaction_type {
            TransactionType::Legacy => {
                if self
                    .access_list
                    .as_ref()
                    .is_some_and(|items| !items.is_empty())
                {
                    return Err(TransactionRequestError::AccessListNotAllowed { transaction_type });
                }

                if has_dynamic_fee {
                    return Err(TransactionRequestError::DynamicFeeNotAllowed { transaction_type });
                }
            }
            TransactionType::AccessList => {
                if has_dynamic_fee {
                    return Err(TransactionRequestError::DynamicFeeNotAllowed { transaction_type });
                }
            }
            TransactionType::DynamicFee => {
                if self.gas_price.is_some() {
                    return Err(TransactionRequestError::GasPriceNotAllowed { transaction_type });
                }
            }
        }

        Ok(())
    }

    pub fn complete(self) -> Result<SimulationTransaction, TransactionRequestError> {
        self.validate_shape()?;
        let transaction_type = self.resolved_type();

        let Self {
            from,
            to,
            nonce,
            gas_limit,
            value,
            input,
            chain_id,
            access_list,
            gas_price,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } = self;

        let nonce = require(nonce, TransactionField::Nonce)?;
        let gas_limit = require(gas_limit, TransactionField::GasLimit)?;
        let chain_id = require(chain_id, TransactionField::ChainId)?;

        let kind = match transaction_type {
            TransactionType::Legacy => TransactionKind::Legacy {
                gas_price: require(gas_price, TransactionField::GasPrice)?,
            },
            TransactionType::AccessList => TransactionKind::AccessList {
                gas_price: require(gas_price, TransactionField::GasPrice)?,
                access_list: access_list.unwrap_or_default(),
            },
            TransactionType::DynamicFee => TransactionKind::DynamicFee {
                max_fee_per_gas: require(max_fee_per_gas, TransactionField::MaxFeePerGas)?,
                max_priority_fee_per_gas: require(
                    max_priority_fee_per_gas,
                    TransactionField::MaxPriorityFeePerGas,
                )?,
                access_list: access_list.unwrap_or_default(),
            },
        };

        Ok(SimulationTransaction {
            from,
            to,
            nonce,
            gas_limit,
            value: value.unwrap_or(U256::ZERO),
            input: input.unwrap_or_default(),
            chain_id,
            kind,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulationTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub input: Bytes,
    pub chain_id: U256,
    pub kind: TransactionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionKind {
    Legacy {
        gas_price: U256,
    },
    AccessList {
        gas_price: U256,
        access_list: Vec<AccessListItem>,
    },
    DynamicFee {
        max_fee_per_gas: U256,
        max_priority_fee_per_gas: U256,
        access_list: Vec<AccessListItem>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionField {
    ChainId,
    Nonce,
    GasLimit,
    GasPrice,
    MaxFeePerGas,
    MaxPriorityFeePerGas,
}

impl fmt::Display for TransactionField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ChainId => "chain_id",
            Self::Nonce => "nonce",
            Self::GasLimit => "gas_limit",
            Self::GasPrice => "gas_price",
            Self::MaxFeePerGas => "max_fee_per_gas",
            Self::MaxPriorityFeePerGas => "max_priority_fee_per_gas",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionRequestError {
    #[error("transaction field `{field}` is required")]
    MissingField { field: TransactionField },

    #[error("{transaction_type} transactions cannot include an access list")]
    AccessListNotAllowed { transaction_type: TransactionType },

    #[error("{transaction_type} transactions cannot include dynamic fee fields")]
    DynamicFeeNotAllowed { transaction_type: TransactionType },

    #[error("{transaction_type} transactions cannot include a gas price")]
    GasPriceNotAllowed { transaction_type: TransactionType },
}

fn require<T>(value: Option<T>, field: TransactionField) -> Result<T, TransactionRequestError> {
    value.ok_or(TransactionRequestError::MissingField { field })
}

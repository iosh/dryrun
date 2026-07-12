use super::{EspaceExecutionFailure, EspaceExecutionFailureCode};
use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceBlockRef {
    Latest,
    Number(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EspaceTransactionVariant {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: u32,
    pub variant: EspaceTransactionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: EspaceBlockRef,
    pub transaction: EspaceTransaction,
}

pub(crate) fn validate_espace_transaction(
    transaction: &EspaceTransaction,
    expected_chain_id: u32,
) -> Result<(), EspaceExecutionFailure> {
    if transaction.chain_id != expected_chain_id {
        return Err(EspaceExecutionFailure {
            code: EspaceExecutionFailureCode::ChainIdMismatch,
            message: format!(
                "transaction chain id {} does not match engine chain id {}",
                transaction.chain_id, expected_chain_id
            ),
            reason: None,
        });
    }

    match &transaction.variant {
        EspaceTransactionVariant::Legacy { gas_price }
        | EspaceTransactionVariant::Eip2930 { gas_price, .. } => {
            if gas_price.is_zero() {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction gas price must be greater than zero".to_string(),
                    reason: None,
                });
            }
        }
        EspaceTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => {
            if max_fee_per_gas.is_zero() {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::ZeroGasPrice,
                    message: "transaction max fee per gas must be greater than zero".to_string(),
                    reason: None,
                });
            }

            if max_priority_fee_per_gas > max_fee_per_gas {
                return Err(EspaceExecutionFailure {
                    code: EspaceExecutionFailureCode::PriorityFeeExceedsMaxFee,
                    message: format!(
                        "max priority fee per gas {} exceeds max fee per gas {}",
                        max_priority_fee_per_gas, max_fee_per_gas
                    ),
                    reason: None,
                });
            }
        }
    }

    Ok(())
}

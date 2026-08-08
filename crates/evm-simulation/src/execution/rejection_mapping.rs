use revm::context_interface::result::InvalidTransaction;

use crate::{
    CompleteTransaction, CompleteTransactionVariant, EvmExecutionError, EvmTransactionRejection,
};

pub(super) fn map_transaction_rejection(
    error: InvalidTransaction,
    transaction: &CompleteTransaction,
    expected_chain_id: u64,
    block_gas_limit: u64,
    base_fee_per_gas: u64,
) -> Result<EvmTransactionRejection, EvmExecutionError> {
    let rejection = match error {
        InvalidTransaction::PriorityFeeGreaterThanMaxFee => {
            let (max_fee_per_gas, max_priority_fee_per_gas) = dynamic_fee_fields(transaction)
                .ok_or_else(|| {
                    EvmExecutionError::unmapped_transaction_validation(
                        "legacy transaction failed dynamic priority fee validation",
                    )
                })?;
            EvmTransactionRejection::PriorityFeeGreaterThanMaxFee {
                max_priority_fee_per_gas,
                max_fee_per_gas,
            }
        }
        InvalidTransaction::GasPriceLessThanBasefee => {
            EvmTransactionRejection::GasPriceBelowBaseFee {
                gas_price: transaction_gas_price(transaction),
                base_fee_per_gas,
            }
        }
        InvalidTransaction::CallerGasLimitMoreThanBlock => {
            EvmTransactionRejection::GasLimitExceedsBlockGasLimit {
                gas_limit: transaction.gas_limit,
                block_gas_limit,
            }
        }
        InvalidTransaction::TxGasLimitGreaterThanCap { gas_limit, cap } => {
            EvmTransactionRejection::GasLimitExceedsCap { gas_limit, cap }
        }
        InvalidTransaction::CallGasCostMoreThanGasLimit {
            initial_gas,
            gas_limit,
        } => EvmTransactionRejection::IntrinsicGasExceedsGasLimit {
            intrinsic_gas: initial_gas,
            gas_limit,
        },
        InvalidTransaction::GasFloorMoreThanGasLimit {
            gas_floor,
            gas_limit,
        } => EvmTransactionRejection::FloorGasExceedsGasLimit {
            floor_gas: gas_floor,
            gas_limit,
        },
        InvalidTransaction::RejectCallerWithCode => EvmTransactionRejection::SenderHasCode {
            sender: transaction.from,
        },
        InvalidTransaction::LackOfFundForMaxFee { fee, balance } => {
            EvmTransactionRejection::InsufficientFunds {
                required: *fee,
                balance: *balance,
            }
        }
        InvalidTransaction::OverflowPaymentInTransaction => {
            EvmTransactionRejection::PaymentOverflow
        }
        InvalidTransaction::NonceOverflowInTransaction => EvmTransactionRejection::NonceOverflow,
        InvalidTransaction::NonceTooHigh { tx, state } => EvmTransactionRejection::NonceTooHigh {
            transaction_nonce: tx,
            state_nonce: state,
        },
        InvalidTransaction::NonceTooLow { tx, state } => EvmTransactionRejection::NonceTooLow {
            transaction_nonce: tx,
            state_nonce: state,
        },
        InvalidTransaction::CreateInitCodeSizeLimit => {
            EvmTransactionRejection::CreateInitCodeSizeLimit
        }
        InvalidTransaction::InvalidChainId => EvmTransactionRejection::InvalidChainId {
            transaction_chain_id: transaction.chain_id,
            expected_chain_id,
        },
        InvalidTransaction::Eip2930NotSupported => EvmTransactionRejection::Eip2930NotActivated,
        InvalidTransaction::Eip1559NotSupported => EvmTransactionRejection::Eip1559NotActivated,
        InvalidTransaction::Eip4844NotSupported => EvmTransactionRejection::Eip4844NotActivated,
        InvalidTransaction::Eip7702NotSupported => EvmTransactionRejection::Eip7702NotActivated,
        error @ (InvalidTransaction::MissingChainId
        | InvalidTransaction::AccessListNotSupported
        | InvalidTransaction::MaxFeePerBlobGasNotSupported
        | InvalidTransaction::BlobVersionedHashesNotSupported
        | InvalidTransaction::BlobGasPriceGreaterThanMax { .. }
        | InvalidTransaction::EmptyBlobs
        | InvalidTransaction::BlobCreateTransaction
        | InvalidTransaction::TooManyBlobs { .. }
        | InvalidTransaction::BlobVersionNotSupported
        | InvalidTransaction::AuthorizationListNotSupported
        | InvalidTransaction::AuthorizationListInvalidFields
        | InvalidTransaction::EmptyAuthorizationList
        | InvalidTransaction::Eip7873NotSupported
        | InvalidTransaction::Eip7873MissingTarget
        | InvalidTransaction::Str(_)) => {
            return Err(EvmExecutionError::unmapped_transaction_validation(
                error.to_string(),
            ));
        }
    };

    Ok(rejection)
}

fn transaction_gas_price(transaction: &CompleteTransaction) -> u128 {
    match transaction.variant {
        CompleteTransactionVariant::Legacy { gas_price }
        | CompleteTransactionVariant::Eip2930 { gas_price, .. } => gas_price,
        CompleteTransactionVariant::Eip1559 {
            max_fee_per_gas, ..
        }
        | CompleteTransactionVariant::Eip4844 {
            max_fee_per_gas, ..
        }
        | CompleteTransactionVariant::Eip7702 {
            max_fee_per_gas, ..
        } => max_fee_per_gas,
    }
}

fn dynamic_fee_fields(transaction: &CompleteTransaction) -> Option<(u128, u128)> {
    match transaction.variant {
        CompleteTransactionVariant::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        }
        | CompleteTransactionVariant::Eip4844 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        }
        | CompleteTransactionVariant::Eip7702 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
            ..
        } => Some((max_fee_per_gas, max_priority_fee_per_gas)),
        CompleteTransactionVariant::Legacy { .. } | CompleteTransactionVariant::Eip2930 { .. } => {
            None
        }
    }
}

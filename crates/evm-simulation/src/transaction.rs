use alloy_primitives::U256;
pub use simulation_core::transaction::{
    AccessListItem, Authorization, DynamicFees, FeeInput, PartialTransactionCommon,
    SignedAuthorization, TransactionCommon, TransactionInput, TransactionInputError,
    TransactionRequest, TxType, TypedTransaction,
};

pub(crate) fn fee_to_u128(
    field: &'static str,
    value: U256,
) -> Result<u128, crate::TransactionInputError> {
    u128::try_from(value).map_err(|_| crate::TransactionInputError::OutOfRange {
        field,
        value,
        maximum: U256::from(u128::MAX),
    })
}

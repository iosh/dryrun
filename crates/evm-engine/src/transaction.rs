pub use simulation_transaction::{
    AccessListItem, Transaction as EvmTransaction, TransactionVariant as EvmTransactionVariant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionInput {
    pub block: crate::ResolvedBlock,
    pub transaction: EvmTransaction,
}

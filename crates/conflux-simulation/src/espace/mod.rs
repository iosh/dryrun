mod changes;
mod completion;
mod context;
mod error;
mod execution;
mod execution_result;
mod outcome_mapping;
mod rejection;
mod result;
mod settlement;
mod simulator;
mod transaction;
mod transaction_adapter;

pub(crate) use changes::EspaceChangesAnalysis;
pub use changes::{EspaceChange, EspaceNativeCurrency};
pub(crate) use completion::complete_transaction;
pub use context::{EspaceBlockContext, EspaceBlockSelector, EspaceContextError};
pub(crate) use context::{ResolvedEspaceContext, resolve_espace_context};
pub use contract_standards::{
    Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, StandardChange,
};
pub(crate) use error::EspaceNativeChangeError;
pub use error::{
    EspaceChangesError, EspaceExecutionError, EspaceResultIntegrationError, EspaceSimulationError,
    EspaceStateAccessError, EspaceTransactionCompletionError,
};
pub use execution::{
    EspaceExecutionFailure, EspaceExecutionOutcome, EspaceLog, EspaceLogAddress,
    EspaceRevertReason, EspaceSuccessOutput,
};
pub use execution_result::{EspaceExecutionResult, EspaceFee, EspaceGas};
pub(crate) use outcome_mapping::convert_executor_outcome;
pub use rejection::EspaceTransactionRejection;
pub use result::EspaceSimulation;
pub(crate) use settlement::verify_fee_settlement;
pub use simulator::EspaceTransactionSimulator;
pub use transaction::{
    AccessListItem, Authorization, EspaceCompleteTransaction, EspaceCompleteTransactionVariant,
    EspacePartialTransaction, EspacePartialTransactionVariant, EspaceTransactionInput,
    EspaceTransactionInputError, SignedAuthorization,
};
pub(crate) use transaction_adapter::{build_executor_transaction, classify_transaction_rejection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceSimulationRequest {
    pub block: EspaceBlockSelector,
    pub transaction: EspaceTransactionInput,
}

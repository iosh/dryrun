//! Chain-independent knowledge shared by the simulators: the execution trace a VM
//! produces, the changes derived from it, execution status, limits and error codes.

mod address;
pub mod changes;
mod error;
mod limits;
mod status;
mod trace;
pub mod transaction;

pub use address::ChainAddress;
pub use changes::{
    ApprovalChange, Asset, BalanceChange, CallResult, ChangeSet, DelegationChange, FeePayment,
    InvolvedContract, StateView, TokenMetadata, TokenReadFailure, derive_changes,
};
pub use error::{CodedError, ErrorCode, ErrorObject};
pub use limits::{LimitExceeded, Limits, ReadBudget, Resource};
pub use status::{ExecutionStatus, Outcome, Rejection, RejectionReason};
pub use trace::{AccountDiff, CallFrame, CallScheme, Diff, ExecutionTrace, Log};

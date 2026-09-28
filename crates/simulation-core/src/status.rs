use alloy_primitives::Bytes;
use serde::{Serialize, Serializer};

use crate::{ChainAddress, ChangeSet, CodedError, ErrorObject};

/// How an executed transaction ended. Only a successful execution has
/// changes; deriving them may fail independently of the execution.
#[derive(Debug, Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    bound(serialize = "A: ChainAddress, E: CodedError")
)]
pub enum ExecutionStatus<A, E> {
    Success {
        output: Bytes,
        #[serde(serialize_with = "serialize_changes")]
        changes: Result<ChangeSet<A>, E>,
    },
    Reverted {
        output: Bytes,
        /// Decoded Solidity `Error(string)` or `Panic(uint256)`.
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Halted {
        reason: String,
    },
}

impl<A, E> ExecutionStatus<A, E> {
    pub fn reverted(output: Bytes) -> Self {
        let reason =
            alloy_sol_types::decode_revert_reason(&output).filter(|reason| !reason.is_empty());
        Self::Reverted { output, reason }
    }
}

fn serialize_changes<A, E, S>(
    changes: &Result<ChangeSet<A>, E>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    A: ChainAddress,
    E: CodedError,
    S: Serializer,
{
    #[derive(Serialize)]
    struct Unavailable {
        error: ErrorObject,
    }

    match changes {
        Ok(changes) => changes.serialize(serializer),
        Err(error) => Unavailable {
            error: error.to_object(),
        }
        .serialize(serializer),
    }
}

/// The VM refused to execute the transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rejection {
    pub reason: RejectionReason,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    NonceTooLow,
    NonceTooHigh,
    NonceMax,
    InsufficientFunds,
    FeeCapTooLow,
    TipAboveFeeCap,
    BlobFeeCapTooLow,
    IntrinsicGasTooLow,
    GasLimitTooHigh,
    SenderNotEoa,
    InvalidChainId,
    InitCodeTooLarge,
    /// Any other violation of the transaction validity rules; see the message.
    InvalidTransaction,
}

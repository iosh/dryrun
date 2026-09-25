use alloy_primitives::Bytes;
use serde::Serialize;

use crate::error::{Diagnostic, ErrorCode, ErrorInfo};

/// Borrows chain accounting, output and logs while encoding the common status contract.
#[derive(Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum OutcomeRef<'a, R, O, L> {
    Success {
        #[serde(flatten)]
        result: &'a R,
        #[serde(flatten)]
        output: &'a O,
        logs: &'a [L],
    },
    Reverted {
        #[serde(flatten)]
        result: &'a R,
        revert_data: &'a Bytes,
        error: Diagnostic,
    },
    Failed {
        #[serde(flatten)]
        result: &'a R,
        error: Diagnostic,
    },
}

pub fn revert_diagnostic(reason: Option<&contract_standards::SolidityRevertReason>) -> Diagnostic {
    reason.map_or_else(
        || ErrorCode::TransactionReverted.diagnostic(),
        ErrorInfo::diagnostic,
    )
}

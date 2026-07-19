//! ERC-721 and ERC-1155 operator approval state checks.

use std::collections::HashMap;

use super::{
    candidate::{ChangeCandidate, ChangeCandidateKind},
    error::TransactionChangesError,
    token_state::{OperatorApprovalKey, TokenStateKeys, TokenStateValues},
};

pub(crate) fn check_operator_approvals(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    let last_approval_values = collect_last_approval_values(candidates);

    for &key in &keys.operator_approvals {
        operator_approval_value(before, key, "before")?;
        let after_approved = operator_approval_value(after, key, "after")?;
        let event_approved = last_approval_values.get(&key).copied().ok_or(
            TransactionChangesError::OperatorApprovalEvidenceMissing {
                collection: key.collection,
                owner: key.owner,
                operator: key.operator,
            },
        )?;

        if event_approved != after_approved {
            return Err(TransactionChangesError::OperatorApprovalValueMismatch {
                collection: key.collection,
                owner: key.owner,
                operator: key.operator,
                event_approved,
                after_approved,
            });
        }
    }

    Ok(())
}

fn collect_last_approval_values(
    candidates: &[ChangeCandidate],
) -> HashMap<OperatorApprovalKey, bool> {
    let mut values = HashMap::new();

    for candidate in candidates {
        let ChangeCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        } = candidate.kind
        else {
            continue;
        };

        values.insert(
            OperatorApprovalKey {
                collection,
                owner,
                operator,
            },
            approved,
        );
    }

    values
}

fn operator_approval_value(
    values: &TokenStateValues,
    key: OperatorApprovalKey,
    state: &'static str,
) -> Result<bool, TransactionChangesError> {
    values.operator_approvals.get(&key).copied().ok_or(
        TransactionChangesError::OperatorApprovalMissing {
            collection: key.collection,
            owner: key.owner,
            operator: key.operator,
            state,
        },
    )
}

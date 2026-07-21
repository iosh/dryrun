//! ERC-721 and ERC-1155 operator approval state checks.

use std::collections::HashMap;

use alloy_primitives::Address;

use crate::{Change, Erc721CollectionMetadata};

use super::{
    PositionedChange,
    candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition},
    error::TransactionChangesError,
    token_state::{CollectionStandards, OperatorApprovalKey, TokenStateKeys, TokenStateValues},
};

#[derive(Debug, Clone, Copy)]
struct PositionedApproval {
    position: ObservationPosition,
    approved: bool,
}

pub(crate) fn check_operator_approvals(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<Vec<PositionedChange>, TransactionChangesError> {
    let last_approval_values = collect_last_approval_values(candidates);
    let mut changes = Vec::new();

    for &key in &keys.operator_approvals {
        let approved_before = operator_approval_value(before, key, "before")?;
        let after_approved = operator_approval_value(after, key, "after")?;
        let event = last_approval_values.get(&key).copied().ok_or(
            TransactionChangesError::OperatorApprovalEvidenceMissing {
                collection: key.collection,
                owner: key.owner,
                operator: key.operator,
            },
        )?;

        if event.approved != after_approved {
            return Err(TransactionChangesError::OperatorApprovalValueMismatch {
                collection: key.collection,
                owner: key.owner,
                operator: key.operator,
                event_approved: event.approved,
                after_approved,
            });
        }

        if approved_before == after_approved {
            continue;
        }

        let standards = collection_standards(before, key.collection)?;
        let change = match (standards.supports_erc721, standards.supports_erc1155) {
            (true, false) => Change::Erc721OperatorApproval {
                contract_address: key.collection,
                owner: key.owner,
                operator: key.operator,
                approved_before,
                approved_after: after_approved,
                metadata: Erc721CollectionMetadata::default(),
            },
            (false, true) => Change::Erc1155OperatorApproval {
                contract_address: key.collection,
                owner: key.owner,
                operator: key.operator,
                approved_before,
                approved_after: after_approved,
            },
            (supports_erc721, supports_erc1155) => {
                return Err(TransactionChangesError::OperatorApprovalStandardAmbiguous {
                    collection: key.collection,
                    supports_erc721,
                    supports_erc1155,
                });
            }
        };

        changes.push(PositionedChange::new(event.position, change));
    }

    Ok(changes)
}

fn collect_last_approval_values(
    candidates: &[ChangeCandidate],
) -> HashMap<OperatorApprovalKey, PositionedApproval> {
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
            PositionedApproval {
                position: candidate.position,
                approved,
            },
        );
    }

    values
}

fn collection_standards(
    values: &TokenStateValues,
    collection: Address,
) -> Result<CollectionStandards, TransactionChangesError> {
    values.collection_standards.get(&collection).copied().ok_or(
        TransactionChangesError::TokenStateValueMissing {
            address: collection,
            value: "before collection standards",
        },
    )
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

//! ERC-721 and ERC-1155 operator approval state checks.

use std::collections::{HashMap, hash_map::Entry};

use alloy_primitives::Address;

use crate::{
    CollectionStandards, ContractStandardsError, OperatorApprovalKey, Position,
    PositionedStandardChange, StandardCandidate, StandardCandidateKind, StandardChange,
    StandardStateValues, StatePhase, StateRequirement,
};

#[derive(Debug, Clone, Copy)]
struct PositionedApproval {
    position: Position,
    approved: bool,
}

pub(crate) fn check_operator_approvals(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedStandardChange>, ContractStandardsError> {
    let last_approval_values = collect_last_approval_values(candidates);
    let mut changes = Vec::new();

    for (key, event) in last_approval_values {
        let approved_before = operator_approval_value(before, key, StatePhase::Before)?;
        let after_approved = operator_approval_value(after, key, StatePhase::After)?;

        if event.approved != after_approved {
            return Err(ContractStandardsError::OperatorApprovalValueMismatch {
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
            (true, false) => StandardChange::Erc721OperatorApproval {
                contract_address: key.collection,
                owner: key.owner,
                operator: key.operator,
                approved_before,
                approved_after: after_approved,
            },
            (false, true) => StandardChange::Erc1155OperatorApproval {
                contract_address: key.collection,
                owner: key.owner,
                operator: key.operator,
                approved_before,
                approved_after: after_approved,
            },
            (supports_erc721, supports_erc1155) => {
                return Err(ContractStandardsError::OperatorApprovalStandardAmbiguous {
                    collection: key.collection,
                    supports_erc721,
                    supports_erc1155,
                });
            }
        };

        changes.push(PositionedStandardChange::new(event.position, change));
    }

    Ok(changes)
}

fn collect_last_approval_values(
    candidates: &[StandardCandidate],
) -> Vec<(OperatorApprovalKey, PositionedApproval)> {
    let mut approval_indexes: HashMap<OperatorApprovalKey, usize> = HashMap::new();
    let mut values: Vec<(OperatorApprovalKey, PositionedApproval)> = Vec::new();

    for candidate in candidates {
        let StandardCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        } = candidate.kind
        else {
            continue;
        };

        let key = OperatorApprovalKey {
            collection,
            owner,
            operator,
        };
        let positioned_approval = PositionedApproval {
            position: candidate.position,
            approved,
        };

        match approval_indexes.entry(key) {
            Entry::Occupied(entry) => values[*entry.get()].1 = positioned_approval,
            Entry::Vacant(entry) => {
                entry.insert(values.len());
                values.push((key, positioned_approval));
            }
        }
    }

    values
}

fn collection_standards(
    values: &StandardStateValues,
    collection: Address,
) -> Result<CollectionStandards, ContractStandardsError> {
    values.collection_standards.get(&collection).copied().ok_or(
        ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::CollectionStandards(collection),
            phase: StatePhase::Before,
        },
    )
}

fn operator_approval_value(
    values: &StandardStateValues,
    key: OperatorApprovalKey,
    phase: StatePhase,
) -> Result<bool, ContractStandardsError> {
    values
        .operator_approvals
        .get(&key)
        .copied()
        .ok_or(ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::OperatorApproval(key),
            phase,
        })
}

use alloy_primitives::Address;

use crate::{
    CollectionStandards, ContractStandardsError, OperatorApprovalKey, Position, StandardCandidate,
    StandardChange, StandardStateValues, candidate::StandardCandidateKind,
};

use super::super::operator_approval::check_operator_approvals;

fn approval(
    observation_index: usize,
    collection: Address,
    owner: Address,
    operator: Address,
    approved: bool,
) -> StandardCandidate {
    StandardCandidate {
        position: Position::new(observation_index, 0),
        kind: StandardCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        },
    }
}

fn state_values(key: OperatorApprovalKey, approved: bool) -> StandardStateValues {
    StandardStateValues {
        collection_standards: [(
            key.collection,
            CollectionStandards {
                supports_erc721: false,
                supports_erc1155: true,
            },
        )]
        .into_iter()
        .collect(),
        operator_approvals: [(key, approved)].into_iter().collect(),
        ..StandardStateValues::default()
    }
}

#[test]
fn checks_last_approval() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let operator = Address::repeat_byte(0x03);
    let key = OperatorApprovalKey {
        collection,
        owner,
        operator,
    };
    let candidates = [
        approval(0, collection, owner, operator, true),
        approval(1, collection, owner, operator, false),
    ];
    let before = state_values(key, true);

    let changes = check_operator_approvals(&candidates, &before, &state_values(key, false))
        .expect("operator approval should reconcile");
    assert_eq!(changes.len(), 1);
    assert!(matches!(
        changes[0].change,
        StandardChange::Erc1155OperatorApproval {
            contract_address,
            owner: change_owner,
            operator: change_operator,
            approved_before: true,
            approved_after: false,
        } if contract_address == collection
            && change_owner == owner
            && change_operator == operator
    ));

    assert!(matches!(
        check_operator_approvals(&candidates, &before, &state_values(key, true)),
        Err(ContractStandardsError::OperatorApprovalValueMismatch { .. })
    ));
}

use alloy_primitives::Address;

use crate::Change;

use super::super::{
    candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition},
    error::TransactionChangesError,
    operator_approval::check_operator_approvals,
    token_state::{
        CollectionStandards, OperatorApprovalKey, TokenStateValues, collect_token_state_keys,
    },
};

fn approval(
    observation_index: usize,
    collection: Address,
    owner: Address,
    operator: Address,
    approved: bool,
) -> ChangeCandidate {
    ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index: 0,
        },
        kind: ChangeCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved,
        },
    }
}

fn state_values(key: OperatorApprovalKey, approved: bool) -> TokenStateValues {
    TokenStateValues {
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
        ..TokenStateValues::default()
    }
}

#[test]
fn uses_last_operator_approval_event_to_check_after_state() {
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
    let keys = collect_token_state_keys(&candidates);
    let before = state_values(key, true);

    let changes = check_operator_approvals(&candidates, &keys, &before, &state_values(key, false))
        .expect("operator approval should reconcile");
    assert_eq!(changes.len(), 1);
    assert!(matches!(
        changes[0].change,
        Change::Erc1155OperatorApproval {
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
        check_operator_approvals(&candidates, &keys, &before, &state_values(key, true)),
        Err(TransactionChangesError::OperatorApprovalValueMismatch { .. })
    ));
}

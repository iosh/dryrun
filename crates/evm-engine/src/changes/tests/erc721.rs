use alloy_primitives::{Address, U256};

use crate::Change;

use super::super::{
    PositionedChange,
    candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition},
    erc721::check_erc721_changes,
    error::TransactionChangesError,
    token_state::{Erc721TokenKey, Erc721TokenState, TokenStateValues, collect_token_state_keys},
};

fn address(byte: u8) -> Address {
    Address::repeat_byte(byte)
}

fn candidate(observation_index: usize, kind: ChangeCandidateKind) -> ChangeCandidate {
    ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index: 0,
        },
        kind,
    }
}

fn movement(
    observation_index: usize,
    collection: Address,
    from: Address,
    to: Address,
    token_id: u64,
) -> ChangeCandidate {
    candidate(
        observation_index,
        ChangeCandidateKind::Erc721Transfer {
            collection,
            from,
            to,
            token_id: U256::from(token_id),
        },
    )
}

fn approval(
    observation_index: usize,
    collection: Address,
    owner: Address,
    approved_address: Option<Address>,
    token_id: u64,
) -> ChangeCandidate {
    candidate(
        observation_index,
        ChangeCandidateKind::Erc721Approval {
            collection,
            owner,
            approved_address,
            token_id: U256::from(token_id),
        },
    )
}

fn present(owner: Address, approved_address: Option<Address>) -> Erc721TokenState {
    Erc721TokenState::Present {
        owner,
        approved_address,
    }
}

fn state_values<const N: usize>(
    collection: Address,
    states: [(u64, Erc721TokenState); N],
) -> TokenStateValues {
    TokenStateValues {
        erc721_tokens: states
            .into_iter()
            .map(|(token_id, state)| {
                (
                    Erc721TokenKey {
                        collection,
                        token_id: U256::from(token_id),
                    },
                    state,
                )
            })
            .collect(),
        ..TokenStateValues::default()
    }
}

fn run_check<const B: usize, const A: usize>(
    collection: Address,
    candidates: &[ChangeCandidate],
    before: [(u64, Erc721TokenState); B],
    after: [(u64, Erc721TokenState); A],
) -> Result<Vec<PositionedChange>, TransactionChangesError> {
    check_erc721_changes(
        candidates,
        &collect_token_state_keys(candidates),
        &state_values(collection, before),
        &state_values(collection, after),
    )
}

#[test]
fn reconciles_ordered_movements_and_approvals() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let operator = address(0x04);
    let candidates = [
        movement(0, collection, alice, alice, 1),
        movement(1, collection, alice, bob, 2),
        approval(2, collection, bob, Some(operator), 2),
    ];

    let changes = run_check(
        collection,
        &candidates,
        [
            (1, present(alice, Some(operator))),
            (2, present(alice, None)),
        ],
        [(1, present(alice, None)), (2, present(bob, Some(operator)))],
    )
    .expect("ERC-721 changes should reconcile");

    assert_eq!(changes.len(), 4);
    assert!(matches!(
        &changes[0].change,
        Change::Erc721Transfer {
            from,
            to,
            token_id,
            ..
        } if from == &alice && to == &alice && token_id == &U256::from(1_u64)
    ));
    assert!(matches!(
        &changes[2].change,
        Change::Erc721TokenApproval {
            approved_address_before: Some(before),
            approved_address_after: None,
            ..
        } if before == &operator
    ));
    assert!(matches!(
        &changes[3].change,
        Change::Erc721TokenApproval {
            approved_address_before: None,
            approved_address_after: Some(after),
            ..
        } if after == &operator
    ));
}

#[test]
fn reconciles_mint_burn_and_remint_path() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let operator = address(0x04);
    let candidates = [
        movement(0, collection, Address::ZERO, alice, 1),
        approval(1, collection, alice, Some(operator), 1),
        movement(2, collection, alice, Address::ZERO, 1),
        movement(3, collection, Address::ZERO, bob, 1),
        movement(4, collection, bob, Address::ZERO, 1),
    ];

    let changes = run_check(
        collection,
        &candidates,
        [(1, Erc721TokenState::OwnerOfReverted)],
        [(1, Erc721TokenState::OwnerOfReverted)],
    )
    .expect("mint, burn, and remint should reconcile");

    assert_eq!(changes.len(), 4);
    assert!(matches!(changes[0].change, Change::Erc721Mint { .. }));
    assert!(matches!(changes[1].change, Change::Erc721Burn { .. }));
    assert!(matches!(changes[2].change, Change::Erc721Mint { .. }));
    assert!(matches!(changes[3].change, Change::Erc721Burn { .. }));
}

#[test]
fn rejects_invalid_transition_paths() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let operator = address(0x04);
    let absent = [(1, Erc721TokenState::OwnerOfReverted)];
    let present = [(1, present(alice, None))];

    assert!(matches!(
        run_check(
            collection,
            &[approval(0, collection, alice, Some(operator), 1)],
            absent,
            absent,
        ),
        Err(TransactionChangesError::Erc721ApprovalInvalid { .. })
    ));

    assert!(matches!(
        run_check(
            collection,
            &[movement(0, collection, bob, operator, 1)],
            present,
            present,
        ),
        Err(TransactionChangesError::Erc721MovementInvalid { .. })
    ));

    assert!(matches!(
        run_check(
            collection,
            &[approval(0, collection, bob, Some(operator), 1)],
            present,
            present,
        ),
        Err(TransactionChangesError::Erc721ApprovalInvalid { .. })
    ));

    assert!(matches!(
        run_check(
            collection,
            &[movement(0, collection, Address::ZERO, Address::ZERO, 1)],
            present,
            present,
        ),
        Err(TransactionChangesError::Erc721MovementInvalid { .. })
    ));
}

#[test]
fn rejects_post_state_that_does_not_match_replay() {
    let collection = address(0x01);
    let alice = address(0x02);
    let operator = address(0x03);
    let present = [(1, present(alice, None))];

    assert!(matches!(
        run_check(
            collection,
            &[movement(0, collection, alice, Address::ZERO, 1)],
            present,
            present,
        ),
        Err(TransactionChangesError::Erc721OwnerMismatch { .. })
    ));

    assert!(matches!(
        run_check(
            collection,
            &[approval(0, collection, alice, Some(operator), 1)],
            present,
            present,
        ),
        Err(TransactionChangesError::Erc721ApprovalMismatch { .. })
    ));
}

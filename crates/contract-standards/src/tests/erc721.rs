use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc721TokenKey, Erc721TokenState, Position, PositionedStandardChange,
    StandardCandidate, StandardChange, StandardStateValues, candidate::StandardCandidateKind,
};

use super::super::erc721::check_erc721_changes;

fn candidate(index: usize, kind: StandardCandidateKind) -> StandardCandidate {
    StandardCandidate {
        position: Position::new(index, 0),
        kind,
    }
}

fn movement(
    index: usize,
    collection: Address,
    from: Address,
    to: Address,
    token_id: u64,
) -> StandardCandidate {
    candidate(
        index,
        StandardCandidateKind::Erc721Transfer {
            collection,
            from,
            to,
            token_id: U256::from(token_id),
        },
    )
}

fn approval(
    index: usize,
    collection: Address,
    owner: Address,
    approved_address: Address,
    token_id: u64,
) -> StandardCandidate {
    candidate(
        index,
        StandardCandidateKind::Erc721Approval {
            collection,
            owner,
            approved_address: Some(approved_address),
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

fn state<const N: usize>(
    collection: Address,
    tokens: [(u64, Erc721TokenState); N],
) -> StandardStateValues {
    StandardStateValues {
        erc721_tokens: tokens
            .into_iter()
            .map(|(token_id, value)| {
                (
                    Erc721TokenKey {
                        collection,
                        token_id: U256::from(token_id),
                    },
                    value,
                )
            })
            .collect(),
        ..StandardStateValues::default()
    }
}

fn check<const B: usize, const A: usize>(
    collection: Address,
    candidates: &[StandardCandidate],
    before: [(u64, Erc721TokenState); B],
    after: [(u64, Erc721TokenState); A],
) -> Result<Vec<PositionedStandardChange>, ContractStandardsError> {
    check_erc721_changes(
        candidates,
        &state(collection, before),
        &state(collection, after),
    )
}

#[test]
fn replays_movements_and_approvals() {
    let collection = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let operator = Address::repeat_byte(0x04);
    let candidates = [
        movement(0, collection, alice, bob, 1),
        movement(1, collection, Address::ZERO, alice, 2),
        approval(2, collection, alice, operator, 2),
        movement(3, collection, bob, Address::ZERO, 3),
    ];

    let changes = check(
        collection,
        &candidates,
        [
            (1, present(alice, Some(operator))),
            (2, Erc721TokenState::OwnerOfReverted),
            (3, present(bob, None)),
        ],
        [
            (1, present(bob, None)),
            (2, present(alice, Some(operator))),
            (3, Erc721TokenState::OwnerOfReverted),
        ],
    )
    .expect("ERC-721 replay");

    assert_eq!(changes.len(), 5);
    assert!(changes.iter().any(|change| matches!(
        &change.change,
        StandardChange::Erc721Transfer { token_id, .. }
            if *token_id == U256::from(1_u64)
    )));
    assert!(changes.iter().any(|change| matches!(
        &change.change,
        StandardChange::Erc721Mint { token_id, .. }
            if *token_id == U256::from(2_u64)
    )));
    assert!(changes.iter().any(|change| matches!(
        &change.change,
        StandardChange::Erc721Burn { token_id, .. }
            if *token_id == U256::from(3_u64)
    )));
    assert!(changes.iter().any(|change| matches!(
        &change.change,
        StandardChange::Erc721TokenApproval {
            approved_address_before: Some(before),
            approved_address_after: None,
            ..
        } if *before == operator
    )));
    assert!(changes.iter().any(|change| matches!(
        &change.change,
        StandardChange::Erc721TokenApproval {
            approved_address_before: None,
            approved_address_after: Some(after),
            ..
        } if *after == operator
    )));
}

#[test]
fn rejects_bad_paths() {
    let collection = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let current = [(1, present(alice, None))];

    assert!(matches!(
        check(
            collection,
            &[movement(0, collection, bob, alice, 1)],
            current,
            current,
        ),
        Err(ContractStandardsError::Erc721MovementInvalid { .. })
    ));
    assert!(matches!(
        check(
            collection,
            &[movement(0, collection, alice, Address::ZERO, 1)],
            current,
            current,
        ),
        Err(ContractStandardsError::Erc721OwnerMismatch { .. })
    ));
}

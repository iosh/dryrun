use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc1155BalanceKey, Position, PositionedStandardChange,
    StandardCandidate, StandardChange, StandardStateValues, candidate::StandardCandidateKind,
    state_requirements,
};

use super::super::erc1155::check_erc1155_movements;

fn address(byte: u8) -> Address {
    Address::repeat_byte(byte)
}

fn movement(
    observation_index: usize,
    collection: Address,
    from: Address,
    to: Address,
    token_id: u64,
    amount: u64,
) -> StandardCandidate {
    StandardCandidate {
        position: Position::new(observation_index, 0),
        kind: StandardCandidateKind::Erc1155Transfer {
            collection,
            from,
            to,
            token_id: U256::from(token_id),
            amount: U256::from(amount),
        },
    }
}

fn state_values<const N: usize>(
    collection: Address,
    balances: [(Address, u64, u64); N],
) -> StandardStateValues {
    StandardStateValues {
        erc1155_balances: balances
            .into_iter()
            .map(|(account, token_id, balance)| {
                (
                    Erc1155BalanceKey {
                        collection,
                        account,
                        token_id: U256::from(token_id),
                    },
                    U256::from(balance),
                )
            })
            .collect(),
        ..StandardStateValues::default()
    }
}

fn run_check(
    candidates: &[StandardCandidate],
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<Vec<PositionedStandardChange>, ContractStandardsError> {
    check_erc1155_movements(candidates, &state_requirements(candidates), before, after)
}

#[test]
fn replays_movements() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let candidates = [
        movement(0, collection, alice, bob, 1, 4),
        movement(1, collection, bob, bob, 1, 5),
        movement(2, collection, bob, Address::ZERO, 1, 2),
        movement(3, collection, Address::ZERO, alice, 1, 3),
    ];

    let changes = run_check(
        &candidates,
        &state_values(collection, [(alice, 1, 10), (bob, 1, 1)]),
        &state_values(collection, [(alice, 1, 9), (bob, 1, 3)]),
    )
    .expect("ERC-1155 movements should reconcile");

    assert_eq!(changes.len(), 4);
    assert!(matches!(
        changes[0].change,
        StandardChange::Erc1155Transfer { .. }
    ));
    assert!(matches!(
        changes[1].change,
        StandardChange::Erc1155Transfer { .. }
    ));
    assert!(matches!(
        changes[2].change,
        StandardChange::Erc1155Burn { .. }
    ));
    assert!(matches!(
        changes[3].change,
        StandardChange::Erc1155Mint { .. }
    ));
}

#[test]
fn rejects_bad_replay() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let transfer = [movement(0, collection, alice, bob, 1, 3)];

    assert!(matches!(
        run_check(
            &transfer,
            &state_values(collection, [(alice, 1, 2), (bob, 1, 0)]),
            &state_values(collection, [(alice, 1, 2), (bob, 1, 0)]),
        ),
        Err(ContractStandardsError::StateArithmetic {
            requirement,
            operation: crate::StateArithmeticOperation::Subtract,
            ..
        }) if matches!(requirement.as_ref(), crate::StateRequirement::Erc1155Balance(_))
    ));

    assert!(matches!(
        run_check(
            &transfer,
            &state_values(collection, [(alice, 1, 10), (bob, 1, 0)]),
            &state_values(collection, [(alice, 1, 8), (bob, 1, 3)]),
        ),
        Err(ContractStandardsError::Erc1155BalanceMismatch { .. })
    ));
}

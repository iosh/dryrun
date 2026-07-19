use alloy_primitives::{Address, U256};

use super::super::{
    candidate::{ChangeCandidate, ChangeCandidateKind, ObservationPosition},
    erc1155::check_erc1155_movements,
    error::TransactionChangesError,
    token_state::{Erc1155BalanceKey, TokenStateValues, collect_token_state_keys},
};

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
) -> ChangeCandidate {
    ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index: 0,
        },
        kind: ChangeCandidateKind::Erc1155Transfer {
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
) -> TokenStateValues {
    TokenStateValues {
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
        ..TokenStateValues::default()
    }
}

fn run_check(
    candidates: &[ChangeCandidate],
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    check_erc1155_movements(
        candidates,
        &collect_token_state_keys(candidates),
        before,
        after,
    )
}

#[test]
fn reconciles_ordered_mint_transfer_self_transfer_and_burn() {
    let collection = address(0x01);
    let alice = address(0x02);
    let bob = address(0x03);
    let candidates = [
        movement(0, collection, alice, bob, 1, 4),
        movement(1, collection, bob, bob, 1, 5),
        movement(2, collection, bob, Address::ZERO, 1, 2),
        movement(3, collection, Address::ZERO, alice, 1, 3),
    ];

    assert_eq!(
        run_check(
            &candidates,
            &state_values(collection, [(alice, 1, 10), (bob, 1, 1)]),
            &state_values(collection, [(alice, 1, 9), (bob, 1, 3)]),
        ),
        Ok(())
    );
}

#[test]
fn rejects_impossible_and_mismatched_balance_paths() {
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
        Err(TransactionChangesError::Erc1155BalanceUnderflow { .. })
    ));

    assert!(matches!(
        run_check(
            &transfer,
            &state_values(collection, [(alice, 1, 10), (bob, 1, 0)]),
            &state_values(collection, [(alice, 1, 8), (bob, 1, 3)]),
        ),
        Err(TransactionChangesError::Erc1155BalanceMismatch { .. })
    ));
}

#[test]
fn accepts_zero_amount_between_zero_addresses_only() {
    let collection = address(0x01);
    let empty = TokenStateValues::default();

    assert_eq!(
        run_check(
            &[movement(0, collection, Address::ZERO, Address::ZERO, 1, 0,)],
            &empty,
            &empty,
        ),
        Ok(())
    );

    assert!(matches!(
        run_check(
            &[movement(0, collection, Address::ZERO, Address::ZERO, 1, 1,)],
            &empty,
            &empty,
        ),
        Err(TransactionChangesError::Erc1155TransferBetweenZeroAddresses { .. })
    ));
}

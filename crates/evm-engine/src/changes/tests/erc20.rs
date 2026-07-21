use alloy_primitives::{Address, U256};

use crate::{Change, Erc20Metadata};

use super::{
    super::{
        erc20::check_erc20_movements,
        error::TransactionChangesError,
        token_state::{Erc20BalanceKey, TokenStateValues, collect_token_state_keys},
    },
    support::erc20_movement_candidate,
};

fn state_values<const N: usize>(
    token: Address,
    balances: [(Address, U256); N],
    total_supply: Option<U256>,
) -> TokenStateValues {
    let mut values = TokenStateValues::default();

    for (account, balance) in balances {
        values
            .erc20_balances
            .insert(Erc20BalanceKey { token, account }, balance);
    }

    if let Some(total_supply) = total_supply {
        values.erc20_total_supplies.insert(token, total_supply);
    }

    values
}

#[test]
fn reconciles_ordered_movements_and_total_supply() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let candidates = [
        erc20_movement_candidate(0, token, alice, alice, U256::from(30_u64)),
        erc20_movement_candidate(1, token, alice, bob, U256::from(40_u64)),
        erc20_movement_candidate(2, token, Address::ZERO, alice, U256::from(10_u64)),
        erc20_movement_candidate(3, token, bob, Address::ZERO, U256::from(5_u64)),
        erc20_movement_candidate(4, token, alice, bob, U256::ZERO),
    ];
    let keys = collect_token_state_keys(&candidates);
    let before = state_values(
        token,
        [(alice, U256::from(100_u64)), (bob, U256::ZERO)],
        Some(U256::from(100_u64)),
    );
    let after = state_values(
        token,
        [(alice, U256::from(70_u64)), (bob, U256::from(35_u64))],
        Some(U256::from(105_u64)),
    );

    let changes = check_erc20_movements(&candidates, &keys, &before, &after)
        .expect("ERC-20 movements should reconcile");

    assert_eq!(
        changes
            .into_iter()
            .map(|positioned_change| positioned_change.change)
            .collect::<Vec<_>>(),
        vec![
            Change::Erc20Transfer {
                contract_address: token,
                from: alice,
                to: alice,
                raw_amount: U256::from(30_u64),
                metadata: Erc20Metadata::default(),
            },
            Change::Erc20Transfer {
                contract_address: token,
                from: alice,
                to: bob,
                raw_amount: U256::from(40_u64),
                metadata: Erc20Metadata::default(),
            },
            Change::Erc20Mint {
                contract_address: token,
                to: alice,
                raw_amount: U256::from(10_u64),
                metadata: Erc20Metadata::default(),
            },
            Change::Erc20Burn {
                contract_address: token,
                from: bob,
                raw_amount: U256::from(5_u64),
                metadata: Erc20Metadata::default(),
            },
        ]
    );
}

#[test]
fn rejects_impossible_movement_paths() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let zero_to_zero = [erc20_movement_candidate(
        0,
        token,
        Address::ZERO,
        Address::ZERO,
        U256::from(1_u64),
    )];

    assert!(matches!(
        check_erc20_movements(
            &zero_to_zero,
            &collect_token_state_keys(&zero_to_zero),
            &TokenStateValues::default(),
            &TokenStateValues::default(),
        ),
        Err(TransactionChangesError::Erc20TransferBetweenZeroAddresses {
            token: invalid_token,
            amount,
        }) if invalid_token == token && amount == U256::from(1_u64)
    ));

    let underflow = [erc20_movement_candidate(
        0,
        token,
        alice,
        bob,
        U256::from(6_u64),
    )];
    let before = state_values(token, [(alice, U256::from(5_u64)), (bob, U256::ZERO)], None);

    assert!(matches!(
        check_erc20_movements(
            &underflow,
            &collect_token_state_keys(&underflow),
            &before,
            &before,
        ),
        Err(TransactionChangesError::Erc20BalanceUnderflow {
            token: underflow_token,
            account,
            balance,
            amount,
        }) if underflow_token == token
            && account == alice
            && balance == U256::from(5_u64)
            && amount == U256::from(6_u64)
    ));
}

#[test]
fn rejects_post_state_that_does_not_match_replay() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let transfer = [erc20_movement_candidate(
        0,
        token,
        alice,
        bob,
        U256::from(4_u64),
    )];
    let before = state_values(
        token,
        [(alice, U256::from(10_u64)), (bob, U256::ZERO)],
        None,
    );
    let wrong_balances = state_values(
        token,
        [(alice, U256::from(7_u64)), (bob, U256::from(4_u64))],
        None,
    );

    assert!(matches!(
        check_erc20_movements(
            &transfer,
            &collect_token_state_keys(&transfer),
            &before,
            &wrong_balances,
        ),
        Err(TransactionChangesError::Erc20BalanceMismatch {
            token: mismatch_token,
            account,
            replayed_balance,
            after_balance,
        }) if mismatch_token == token
            && account == alice
            && replayed_balance == U256::from(6_u64)
            && after_balance == U256::from(7_u64)
    ));

    let mint = [erc20_movement_candidate(
        0,
        token,
        Address::ZERO,
        alice,
        U256::from(5_u64),
    )];
    let before = state_values(token, [(alice, U256::ZERO)], Some(U256::from(100_u64)));
    let wrong_supply = state_values(
        token,
        [(alice, U256::from(5_u64))],
        Some(U256::from(106_u64)),
    );

    assert!(matches!(
        check_erc20_movements(
            &mint,
            &collect_token_state_keys(&mint),
            &before,
            &wrong_supply,
        ),
        Err(TransactionChangesError::Erc20TotalSupplyMismatch {
            token: mismatch_token,
            replayed_total_supply,
            after_total_supply,
        }) if mismatch_token == token
            && replayed_total_supply == U256::from(105_u64)
            && after_total_supply == U256::from(106_u64)
    ));
}

use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc20BalanceKey, StandardChange, StandardStateValues,
    state_requirements,
};

use super::{super::erc20::check_erc20_movements, support::erc20_movement_candidate};

fn state<const N: usize>(
    token: Address,
    balances: [(Address, u64); N],
    supply: Option<u64>,
) -> StandardStateValues {
    let mut values = StandardStateValues::default();
    for (account, balance) in balances {
        values
            .erc20_balances
            .insert(Erc20BalanceKey { token, account }, U256::from(balance));
    }
    if let Some(supply) = supply {
        values
            .erc20_total_supplies
            .insert(token, U256::from(supply));
    }
    values
}

#[test]
fn checks_replay() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let candidates = [
        erc20_movement_candidate(0, token, alice, bob, U256::from(20_u64)),
        erc20_movement_candidate(1, token, Address::ZERO, alice, U256::from(5_u64)),
        erc20_movement_candidate(2, token, bob, Address::ZERO, U256::from(3_u64)),
        erc20_movement_candidate(3, token, alice, bob, U256::ZERO),
    ];

    let changes = check_erc20_movements(
        &candidates,
        &state_requirements(&candidates),
        &state(token, [(alice, 100), (bob, 10)], Some(110)),
        &state(token, [(alice, 85), (bob, 27)], Some(112)),
    )
    .expect("replay");

    assert_eq!(changes.len(), 3);
    assert!(matches!(
        changes[0].change,
        StandardChange::Erc20Transfer { .. }
    ));
    assert!(matches!(
        changes[1].change,
        StandardChange::Erc20Mint { .. }
    ));
    assert!(matches!(
        changes[2].change,
        StandardChange::Erc20Burn { .. }
    ));
}

#[test]
fn rejects_invalid_replay() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let candidates = [erc20_movement_candidate(
        0,
        token,
        alice,
        bob,
        U256::from(6_u64),
    )];
    let requirements = state_requirements(&candidates);

    assert!(matches!(
        check_erc20_movements(
            &candidates,
            &requirements,
            &state(token, [(alice, 5), (bob, 0)], None),
            &state(token, [(alice, 0), (bob, 6)], None),
        ),
        Err(ContractStandardsError::StateArithmetic { .. })
    ));

    assert!(matches!(
        check_erc20_movements(
            &candidates,
            &requirements,
            &state(token, [(alice, 10), (bob, 0)], None),
            &state(token, [(alice, 5), (bob, 6)], None),
        ),
        Err(ContractStandardsError::Erc20BalanceMismatch { .. })
    ));
}

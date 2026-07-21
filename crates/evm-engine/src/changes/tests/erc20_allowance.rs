use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::Change;

use super::{
    super::{
        candidate::{ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence},
        erc20::check_erc20_allowances,
        error::TransactionChangesError,
        token_state::{Erc20AllowanceKey, TokenStateValues, collect_token_state_keys},
    },
    support::candidate,
};

fn allowance_candidate(
    observation_index: usize,
    token: Address,
    owner: Address,
    spender: Address,
    evidence: Erc20AllowanceEvidence,
) -> ChangeCandidate {
    candidate(
        observation_index,
        0,
        ChangeCandidateKind::Erc20Allowance {
            token,
            owner,
            spender,
            evidence,
        },
    )
}

fn state_values<const N: usize>(allowances: [(Erc20AllowanceKey, U256); N]) -> TokenStateValues {
    TokenStateValues {
        erc20_allowances: HashMap::from(allowances),
        ..TokenStateValues::default()
    }
}

#[test]
fn uses_last_evidence_to_check_allowances() {
    let token = Address::repeat_byte(0x01);
    let alice = Address::repeat_byte(0x02);
    let bob = Address::repeat_byte(0x03);
    let spender = Address::repeat_byte(0x04);
    let alice_key = Erc20AllowanceKey {
        token,
        owner: alice,
        spender,
    };
    let bob_key = Erc20AllowanceKey {
        token,
        owner: bob,
        spender,
    };
    let candidates = [
        allowance_candidate(
            0,
            token,
            alice,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(60_u64),
            },
        ),
        allowance_candidate(
            1,
            token,
            bob,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(20_u64),
            },
        ),
        allowance_candidate(
            2,
            token,
            alice,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(70_u64),
            },
        ),
        allowance_candidate(
            3,
            token,
            bob,
            spender,
            Erc20AllowanceEvidence::TransferFromCall {
                amount: U256::from(5_u64),
            },
        ),
    ];
    let keys = collect_token_state_keys(&candidates);
    let before = state_values([
        (alice_key, U256::from(10_u64)),
        (bob_key, U256::from(20_u64)),
    ]);
    let after = state_values([
        (alice_key, U256::from(70_u64)),
        (bob_key, U256::from(19_u64)),
    ]);

    let changes = check_erc20_allowances(&candidates, &keys, &before, &after)
        .expect("ERC-20 allowances should reconcile");

    assert_eq!(changes.len(), 2);
    assert!(matches!(
        changes[0].change,
        Change::Erc20Allowance {
            contract_address,
            owner,
            spender: change_spender,
            raw_amount_before,
            raw_amount_after,
            ..
        } if contract_address == token
            && owner == alice
            && change_spender == spender
            && raw_amount_before == U256::from(10_u64)
            && raw_amount_after == U256::from(70_u64)
    ));
    assert_eq!(changes[0].position.observation_index, 2);
    assert!(matches!(
        changes[1].change,
        Change::Erc20Allowance {
            owner,
            raw_amount_before,
            raw_amount_after,
            ..
        } if owner == bob
            && raw_amount_before == U256::from(20_u64)
            && raw_amount_after == U256::from(19_u64)
    ));
    assert_eq!(changes[1].position.observation_index, 3);
}

#[test]
fn rejects_last_approval_that_disagrees_with_after_state() {
    let token = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let spender = Address::repeat_byte(0x03);
    let key = Erc20AllowanceKey {
        token,
        owner,
        spender,
    };
    let candidates = [
        allowance_candidate(
            0,
            token,
            owner,
            spender,
            Erc20AllowanceEvidence::TransferFromCall {
                amount: U256::from(5_u64),
            },
        ),
        allowance_candidate(
            1,
            token,
            owner,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(30_u64),
            },
        ),
    ];
    let before = state_values([(key, U256::from(40_u64))]);
    let after = state_values([(key, U256::from(29_u64))]);

    assert!(matches!(
        check_erc20_allowances(
            &candidates,
            &collect_token_state_keys(&candidates),
            &before,
            &after,
        ),
        Err(TransactionChangesError::Erc20ApprovalValueMismatch {
            token: mismatch_token,
            owner: mismatch_owner,
            spender: mismatch_spender,
            event_value,
            after_allowance,
        }) if mismatch_token == token
            && mismatch_owner == owner
            && mismatch_spender == spender
            && event_value == U256::from(30_u64)
            && after_allowance == U256::from(29_u64)
    ));
}

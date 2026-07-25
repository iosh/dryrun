use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::{
    ContractStandardsError, Erc20AllowanceKey, StandardCandidate, StandardChange,
    StandardStateValues,
    candidate::{AllowanceSource, StandardCandidateKind},
};

use super::{super::erc20::check_erc20_allowances, support::candidate};

fn allowance(
    index: usize,
    token: Address,
    owner: Address,
    spender: Address,
    source: AllowanceSource,
) -> StandardCandidate {
    candidate(
        index,
        0,
        StandardCandidateKind::Erc20Allowance {
            token,
            owner,
            spender,
            source,
        },
    )
}

fn state(key: Erc20AllowanceKey, value: u64) -> StandardStateValues {
    StandardStateValues {
        erc20_allowances: HashMap::from([(key, U256::from(value))]),
        ..StandardStateValues::default()
    }
}

#[test]
fn checks_last_allowance_source() {
    let token = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let spender = Address::repeat_byte(0x03);
    let key = Erc20AllowanceKey {
        token,
        owner,
        spender,
    };
    let candidates = [
        allowance(
            0,
            token,
            owner,
            spender,
            AllowanceSource::ApprovalEvent {
                value: U256::from(20_u64),
            },
        ),
        allowance(
            1,
            token,
            owner,
            spender,
            AllowanceSource::TransferFromCall {
                amount: U256::from(5_u64),
            },
        ),
    ];

    let changes = check_erc20_allowances(&candidates, &state(key, 20), &state(key, 19))
        .expect("allowance replay");

    assert!(matches!(
        &changes[..],
        [change] if change.position.index == 1 && matches!(
            change.change,
            StandardChange::Erc20Allowance {
                raw_amount_before,
                raw_amount_after,
                ..
            } if raw_amount_before == U256::from(20_u64)
                && raw_amount_after == U256::from(19_u64)
        )
    ));

    let approval = [allowance(
        0,
        token,
        owner,
        spender,
        AllowanceSource::ApprovalEvent {
            value: U256::from(30_u64),
        },
    )];
    assert!(matches!(
        check_erc20_allowances(&approval, &state(key, 20), &state(key, 29)),
        Err(ContractStandardsError::Erc20ApprovalValueMismatch { .. })
    ));
}

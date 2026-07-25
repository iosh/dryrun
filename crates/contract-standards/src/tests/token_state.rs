use alloy_primitives::{Address, U256};

use crate::{
    Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc1155BalanceKey, OperatorApprovalKey,
    StateRequirements,
    candidate::{AllowanceSource, StandardCandidateKind},
    state_requirements,
};

use super::support::{candidate, erc20_movement_candidate};

#[test]
fn orders_requirements() {
    let erc20 = Address::repeat_byte(0x01);
    let erc721 = Address::repeat_byte(0x02);
    let erc1155 = Address::repeat_byte(0x03);
    let owner = Address::repeat_byte(0x04);
    let recipient = Address::repeat_byte(0x05);
    let spender = Address::repeat_byte(0x06);
    let operator = Address::repeat_byte(0x07);
    let erc721_token_id = U256::from(11_u64);
    let erc1155_token_id = U256::from(12_u64);
    let candidates = [
        erc20_movement_candidate(0, erc20, owner, recipient, U256::from(2_u64)),
        erc20_movement_candidate(1, erc20, owner, Address::ZERO, U256::from(3_u64)),
        candidate(
            2,
            0,
            StandardCandidateKind::Erc20Allowance {
                token: erc20,
                owner,
                spender,
                source: AllowanceSource::ApprovalEvent {
                    value: U256::from(4_u64),
                },
            },
        ),
        candidate(
            3,
            0,
            StandardCandidateKind::Erc721Transfer {
                collection: erc721,
                from: owner,
                to: recipient,
                token_id: erc721_token_id,
            },
        ),
        candidate(
            4,
            0,
            StandardCandidateKind::Erc721Approval {
                collection: erc721,
                owner,
                approved_address: Some(spender),
                token_id: erc721_token_id,
            },
        ),
        candidate(
            5,
            0,
            StandardCandidateKind::Erc1155Transfer {
                collection: erc1155,
                from: Address::ZERO,
                to: recipient,
                token_id: erc1155_token_id,
                amount: U256::from(5_u64),
            },
        ),
        candidate(
            6,
            0,
            StandardCandidateKind::OperatorApproval {
                collection: erc1155,
                owner,
                operator,
                approved: true,
            },
        ),
    ];

    assert_eq!(
        state_requirements(&candidates),
        StateRequirements {
            token_contracts: vec![erc20, erc721, erc1155],
            collection_standards: vec![erc721, erc1155],
            erc20_balances: vec![
                Erc20BalanceKey {
                    token: erc20,
                    account: owner,
                },
                Erc20BalanceKey {
                    token: erc20,
                    account: recipient,
                },
            ],
            erc20_total_supplies: vec![erc20],
            erc20_allowances: vec![Erc20AllowanceKey {
                token: erc20,
                owner,
                spender,
            }],
            erc721_tokens: vec![Erc721TokenKey {
                collection: erc721,
                token_id: erc721_token_id,
            }],
            erc1155_balances: vec![Erc1155BalanceKey {
                collection: erc1155,
                account: recipient,
                token_id: erc1155_token_id,
            }],
            operator_approvals: vec![OperatorApprovalKey {
                collection: erc1155,
                owner,
                operator,
            }],
        }
    );
}

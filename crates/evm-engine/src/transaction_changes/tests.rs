mod erc1155;
mod erc721;
mod operator_approval;

use std::collections::HashMap;

use alloy::sol_types::SolValue;
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use revm::{
    Context, InspectEvm, MainBuilder, MainContext,
    context::TxEnv,
    database::InMemoryDB,
    primitives::TxKind,
    state::{Account, AccountInfo, EvmState},
};

use crate::change_observation::{ChangeObservationInspector, Observation};

use super::{
    candidate::{
        ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence, ObservationPosition,
    },
    change_data::{
        ChangeDataRequests, Erc721CollectionMetadataRequest, collect_change_data_requests,
    },
    collection::collect_candidates,
    erc20::{check_erc20_allowances, check_erc20_movements},
    error::TransactionChangesError,
    event_codec::SupportedEvent,
    native_balance::check_native_balances,
    token_contract::check_token_contracts,
    token_state::{
        CollectionStandards, Erc20AllowanceKey, Erc20BalanceKey, Erc721TokenKey, Erc1155BalanceKey,
        OperatorApprovalKey, TokenStateKeys, TokenStateValues, collect_token_state_keys,
    },
};

fn candidate(
    observation_index: usize,
    item_index: usize,
    kind: ChangeCandidateKind,
) -> ChangeCandidate {
    ChangeCandidate {
        position: ObservationPosition {
            observation_index,
            item_index,
        },
        kind,
    }
}

fn state_account(original_balance: U256, current_balance: U256) -> Account {
    let mut account = Account::from(AccountInfo::default().with_balance(original_balance));
    account.info.balance = current_balance;
    account.mark_touch();
    account
}

fn native_state<const N: usize>(accounts: [(Address, U256, U256); N]) -> EvmState {
    accounts
        .into_iter()
        .map(|(address, original_balance, current_balance)| {
            (address, state_account(original_balance, current_balance))
        })
        .collect()
}

fn native_candidate(
    observation_index: usize,
    from: Address,
    to: Address,
    amount: U256,
) -> ChangeCandidate {
    candidate(
        observation_index,
        0,
        ChangeCandidateKind::NativeTransfer { from, to, amount },
    )
}

fn erc20_movement_candidate(
    observation_index: usize,
    token: Address,
    from: Address,
    to: Address,
    amount: U256,
) -> ChangeCandidate {
    candidate(
        observation_index,
        0,
        ChangeCandidateKind::Erc20Transfer {
            token,
            from,
            to,
            amount,
        },
    )
}

fn erc20_allowance_candidate(
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

fn token_state_values(contract: Address, standards: CollectionStandards) -> TokenStateValues {
    let mut values = TokenStateValues::default();
    values
        .contract_code_hashes
        .insert(contract, B256::repeat_byte(0x11));
    values.collection_standards.insert(contract, standards);
    values
}

fn erc20_state_values<const N: usize>(
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

fn erc20_allowance_state_values<const N: usize>(
    allowances: [(Erc20AllowanceKey, U256); N],
) -> TokenStateValues {
    TokenStateValues {
        erc20_allowances: HashMap::from(allowances),
        ..TokenStateValues::default()
    }
}

fn indexed_address(address: Address) -> B256 {
    address.into_word()
}

fn indexed_u256(value: U256) -> B256 {
    B256::from(value.to_be_bytes::<32>())
}

fn event_topic(signature: &str) -> B256 {
    keccak256(signature)
}

fn call_observation(caller: Address, target: Address, value: u64) -> Observation {
    Observation::Call {
        caller,
        target,
        value: U256::from(value),
        input_len: 0,
        input_prefix: Bytes::new(),
    }
}

fn erc20_transfer_observation(
    token: Address,
    from: Address,
    to: Address,
    amount: u64,
) -> Observation {
    Observation::Log {
        address: token,
        topics: vec![
            event_topic("Transfer(address,address,uint256)"),
            indexed_address(from),
            indexed_address(to),
        ],
        data: Bytes::from(U256::from(amount).to_be_bytes_vec()),
    }
}

fn erc721_transfer_observation(
    collection: Address,
    from: Address,
    to: Address,
    token_id: u64,
) -> Observation {
    Observation::Log {
        address: collection,
        topics: vec![
            event_topic("Transfer(address,address,uint256)"),
            indexed_address(from),
            indexed_address(to),
            indexed_u256(U256::from(token_id)),
        ],
        data: Bytes::new(),
    }
}

fn erc20_approval_observation(
    token: Address,
    owner: Address,
    spender: Address,
    value: u64,
) -> Observation {
    Observation::Log {
        address: token,
        topics: vec![
            event_topic("Approval(address,address,uint256)"),
            indexed_address(owner),
            indexed_address(spender),
        ],
        data: Bytes::from(U256::from(value).to_be_bytes_vec()),
    }
}

fn erc721_approval_observation(
    collection: Address,
    owner: Address,
    approved_address: Address,
    token_id: u64,
) -> Observation {
    Observation::Log {
        address: collection,
        topics: vec![
            event_topic("Approval(address,address,uint256)"),
            indexed_address(owner),
            indexed_address(approved_address),
            indexed_u256(U256::from(token_id)),
        ],
        data: Bytes::new(),
    }
}

fn approval_for_all_observation(
    collection: Address,
    owner: Address,
    operator: Address,
    approved_word: U256,
) -> Observation {
    Observation::Log {
        address: collection,
        topics: vec![
            event_topic("ApprovalForAll(address,address,bool)"),
            indexed_address(owner),
            indexed_address(operator),
        ],
        data: Bytes::from(approved_word.to_be_bytes_vec()),
    }
}

fn transfer_from_call_observation(
    caller: Address,
    token: Address,
    owner: Address,
    recipient: Address,
    amount: u64,
) -> Observation {
    let signature_hash = keccak256("transferFrom(address,address,uint256)");
    let mut input = Vec::with_capacity(100);
    input.extend_from_slice(&signature_hash[..4]);
    input.extend_from_slice(indexed_address(owner).as_slice());
    input.extend_from_slice(indexed_address(recipient).as_slice());
    input.extend_from_slice(&U256::from(amount).to_be_bytes::<32>());

    Observation::Call {
        caller,
        target: token,
        value: U256::ZERO,
        input_len: input.len(),
        input_prefix: Bytes::from(input),
    }
}

fn erc1155_transfer_single_observation(
    collection: Address,
    operator: Address,
    from: Address,
    to: Address,
    token_id: u64,
    amount: u64,
) -> Observation {
    Observation::Log {
        address: collection,
        topics: vec![
            event_topic("TransferSingle(address,address,address,uint256,uint256)"),
            indexed_address(operator),
            indexed_address(from),
            indexed_address(to),
        ],
        data: Bytes::from((U256::from(token_id), U256::from(amount)).abi_encode_sequence()),
    }
}

fn erc1155_transfer_batch_observation(
    collection: Address,
    operator: Address,
    from: Address,
    to: Address,
    token_ids: &[u64],
    amounts: &[u64],
) -> Observation {
    Observation::Log {
        address: collection,
        topics: vec![
            event_topic("TransferBatch(address,address,address,uint256[],uint256[])"),
            indexed_address(operator),
            indexed_address(from),
            indexed_address(to),
        ],
        data: Bytes::from(
            (
                token_ids
                    .iter()
                    .copied()
                    .map(U256::from)
                    .collect::<Vec<_>>(),
                amounts.iter().copied().map(U256::from).collect::<Vec<_>>(),
            )
                .abi_encode_sequence(),
        ),
    }
}

#[test]
fn collects_deduplicated_change_data_requests_in_candidate_order() {
    let operator_only = Address::repeat_byte(0x01);
    let operator_then_erc721 = Address::repeat_byte(0x02);
    let erc20 = Address::repeat_byte(0x03);
    let transfer_from_only = Address::repeat_byte(0x04);
    let owner = Address::repeat_byte(0x05);
    let operator = Address::repeat_byte(0x06);
    let recipient = Address::repeat_byte(0x07);

    let requests = collect_change_data_requests(&[
        candidate(
            0,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: operator_only,
                owner,
                operator,
                approved: true,
            },
        ),
        candidate(
            1,
            0,
            ChangeCandidateKind::Erc20Transfer {
                token: erc20,
                from: owner,
                to: recipient,
                amount: U256::from(1_u64),
            },
        ),
        candidate(
            2,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: operator_then_erc721,
                owner,
                operator,
                approved: true,
            },
        ),
        candidate(
            3,
            0,
            ChangeCandidateKind::Erc20Allowance {
                token: erc20,
                owner,
                spender: operator,
                evidence: Erc20AllowanceEvidence::ApprovalEvent {
                    value: U256::from(2_u64),
                },
            },
        ),
        candidate(
            4,
            0,
            ChangeCandidateKind::Erc721Approval {
                collection: operator_then_erc721,
                owner,
                approved_address: Some(operator),
                token_id: U256::from(3_u64),
            },
        ),
        candidate(
            5,
            0,
            ChangeCandidateKind::Erc20Allowance {
                token: transfer_from_only,
                owner,
                spender: operator,
                evidence: Erc20AllowanceEvidence::TransferFromCall {
                    amount: U256::from(4_u64),
                },
            },
        ),
        candidate(
            6,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: operator_only,
                owner,
                operator,
                approved: false,
            },
        ),
    ]);

    assert_eq!(
        requests,
        ChangeDataRequests {
            contract_kinds: vec![operator_only, operator_then_erc721],
            erc20_metadata: vec![erc20],
            erc721_collection_metadata: vec![
                Erc721CollectionMetadataRequest {
                    collection: operator_only,
                    only_if_classified_as_erc721: true,
                },
                Erc721CollectionMetadataRequest {
                    collection: operator_then_erc721,
                    only_if_classified_as_erc721: false,
                },
            ],
        }
    );
}

#[test]
fn collects_deduplicated_token_state_keys_in_candidate_order() {
    let erc20 = Address::repeat_byte(0x01);
    let erc721 = Address::repeat_byte(0x02);
    let erc1155 = Address::repeat_byte(0x03);
    let owner = Address::repeat_byte(0x04);
    let recipient = Address::repeat_byte(0x05);
    let spender = Address::repeat_byte(0x06);
    let operator = Address::repeat_byte(0x07);
    let erc721_token_id = U256::from(11_u64);
    let erc1155_token_id = U256::from(12_u64);

    let candidates = collect_candidates(&[
        call_observation(owner, recipient, 1),
        erc20_transfer_observation(erc20, owner, recipient, 2),
        erc20_transfer_observation(erc20, owner, Address::ZERO, 3),
        erc20_approval_observation(erc20, owner, spender, 4),
        erc721_transfer_observation(erc721, owner, recipient, 11),
        erc721_approval_observation(erc721, owner, spender, 11),
        erc1155_transfer_single_observation(erc1155, operator, Address::ZERO, recipient, 12, 5),
        approval_for_all_observation(erc1155, owner, operator, U256::from(1_u64)),
    ])
    .expect("token state candidates");

    let keys = collect_token_state_keys(&candidates);

    assert_eq!(
        keys,
        TokenStateKeys {
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

#[test]
fn reconciles_ordered_erc20_movements_and_total_supply() {
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
    let before = erc20_state_values(
        token,
        [(alice, U256::from(100_u64)), (bob, U256::ZERO)],
        Some(U256::from(100_u64)),
    );
    let after = erc20_state_values(
        token,
        [(alice, U256::from(70_u64)), (bob, U256::from(35_u64))],
        Some(U256::from(105_u64)),
    );

    assert_eq!(
        check_erc20_movements(&candidates, &keys, &before, &after),
        Ok(())
    );
}

#[test]
fn rejects_impossible_erc20_movement_paths() {
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
    let zero_to_zero_keys = collect_token_state_keys(&zero_to_zero);
    assert!(matches!(
        check_erc20_movements(
            &zero_to_zero,
            &zero_to_zero_keys,
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
    let underflow_keys = collect_token_state_keys(&underflow);
    let before = erc20_state_values(token, [(alice, U256::from(5_u64)), (bob, U256::ZERO)], None);
    assert!(matches!(
        check_erc20_movements(&underflow, &underflow_keys, &before, &before),
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
fn rejects_erc20_post_state_that_does_not_match_replay() {
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
    let transfer_keys = collect_token_state_keys(&transfer);
    let before = erc20_state_values(
        token,
        [(alice, U256::from(10_u64)), (bob, U256::ZERO)],
        None,
    );
    let wrong_balances = erc20_state_values(
        token,
        [(alice, U256::from(7_u64)), (bob, U256::from(4_u64))],
        None,
    );
    assert!(matches!(
        check_erc20_movements(&transfer, &transfer_keys, &before, &wrong_balances),
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
    let mint_keys = collect_token_state_keys(&mint);
    let before = erc20_state_values(token, [(alice, U256::ZERO)], Some(U256::from(100_u64)));
    let wrong_supply = erc20_state_values(
        token,
        [(alice, U256::from(5_u64))],
        Some(U256::from(106_u64)),
    );
    assert!(matches!(
        check_erc20_movements(&mint, &mint_keys, &before, &wrong_supply),
        Err(TransactionChangesError::Erc20TotalSupplyMismatch {
            token: mismatch_token,
            replayed_total_supply,
            after_total_supply,
        }) if mismatch_token == token
            && replayed_total_supply == U256::from(105_u64)
            && after_total_supply == U256::from(106_u64)
    ));
}

#[test]
fn uses_last_evidence_to_check_erc20_allowances() {
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
        erc20_allowance_candidate(
            0,
            token,
            alice,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(60_u64),
            },
        ),
        erc20_allowance_candidate(
            1,
            token,
            bob,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(20_u64),
            },
        ),
        erc20_allowance_candidate(
            2,
            token,
            alice,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(70_u64),
            },
        ),
        erc20_allowance_candidate(
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
    let before = erc20_allowance_state_values([
        (alice_key, U256::from(10_u64)),
        (bob_key, U256::from(20_u64)),
    ]);
    let after = erc20_allowance_state_values([
        (alice_key, U256::from(70_u64)),
        (bob_key, U256::from(19_u64)),
    ]);

    assert_eq!(
        check_erc20_allowances(&candidates, &keys, &before, &after),
        Ok(())
    );
}

#[test]
fn rejects_erc20_allowance_when_last_approval_disagrees_with_after_state() {
    let token = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let spender = Address::repeat_byte(0x03);
    let key = Erc20AllowanceKey {
        token,
        owner,
        spender,
    };
    let candidates = [
        erc20_allowance_candidate(
            0,
            token,
            owner,
            spender,
            Erc20AllowanceEvidence::TransferFromCall {
                amount: U256::from(5_u64),
            },
        ),
        erc20_allowance_candidate(
            1,
            token,
            owner,
            spender,
            Erc20AllowanceEvidence::ApprovalEvent {
                value: U256::from(30_u64),
            },
        ),
    ];
    let keys = collect_token_state_keys(&candidates);
    let before = erc20_allowance_state_values([(key, U256::from(40_u64))]);
    let after = erc20_allowance_state_values([(key, U256::from(29_u64))]);

    assert!(matches!(
        check_erc20_allowances(&candidates, &keys, &before, &after),
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

#[test]
fn requires_token_code_and_collection_standards_to_remain_stable() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let recipient = Address::repeat_byte(0x03);
    let candidates = [candidate(
        0,
        0,
        ChangeCandidateKind::Erc721Transfer {
            collection,
            from: owner,
            to: recipient,
            token_id: U256::from(1_u64),
        },
    )];
    let keys = collect_token_state_keys(&candidates);
    let before = token_state_values(
        collection,
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: false,
        },
    );
    let mut after = before.clone();

    assert_eq!(
        check_token_contracts(&candidates, &keys, &before, &after),
        Ok(())
    );

    after
        .contract_code_hashes
        .insert(collection, B256::repeat_byte(0x22));
    assert!(matches!(
        check_token_contracts(&candidates, &keys, &before, &after),
        Err(TransactionChangesError::TokenContractCodeChanged {
            contract,
            ..
        }) if contract == collection
    ));

    after = before.clone();
    after.collection_standards.insert(
        collection,
        CollectionStandards {
            supports_erc721: false,
            supports_erc1155: true,
        },
    );
    assert!(matches!(
        check_token_contracts(&candidates, &keys, &before, &after),
        Err(TransactionChangesError::CollectionStandardsChanged {
            collection: changed_collection,
            ..
        }) if changed_collection == collection
    ));
}

#[test]
fn requires_supported_and_unambiguous_collection_standards() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let recipient = Address::repeat_byte(0x03);
    let operator = Address::repeat_byte(0x04);
    let erc721_candidates = [candidate(
        0,
        0,
        ChangeCandidateKind::Erc721Transfer {
            collection,
            from: owner,
            to: recipient,
            token_id: U256::from(1_u64),
        },
    )];
    let erc721_keys = collect_token_state_keys(&erc721_candidates);

    let supports_both = token_state_values(
        collection,
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: true,
        },
    );
    assert_eq!(
        check_token_contracts(
            &erc721_candidates,
            &erc721_keys,
            &supports_both,
            &supports_both,
        ),
        Ok(())
    );

    let erc1155_only = token_state_values(
        collection,
        CollectionStandards {
            supports_erc721: false,
            supports_erc1155: true,
        },
    );
    assert!(matches!(
        check_token_contracts(
            &erc721_candidates,
            &erc721_keys,
            &erc1155_only,
            &erc1155_only,
        ),
        Err(TransactionChangesError::CollectionStandardNotSupported {
            collection: unsupported_collection,
            standard: "ERC-721",
        }) if unsupported_collection == collection
    ));

    let operator_candidates = [candidate(
        0,
        0,
        ChangeCandidateKind::OperatorApproval {
            collection,
            owner,
            operator,
            approved: true,
        },
    )];
    let operator_keys = collect_token_state_keys(&operator_candidates);

    for standards in [
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: false,
        },
        CollectionStandards {
            supports_erc721: false,
            supports_erc1155: true,
        },
    ] {
        let values = token_state_values(collection, standards);
        assert_eq!(
            check_token_contracts(&operator_candidates, &operator_keys, &values, &values,),
            Ok(())
        );
    }

    for standards in [
        CollectionStandards {
            supports_erc721: false,
            supports_erc1155: false,
        },
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: true,
        },
    ] {
        let values = token_state_values(collection, standards);
        assert!(matches!(
            check_token_contracts(
                &operator_candidates,
                &operator_keys,
                &values,
                &values,
            ),
            Err(TransactionChangesError::OperatorApprovalStandardAmbiguous {
                collection: ambiguous_collection,
                ..
            }) if ambiguous_collection == collection
        ));
    }
}

#[test]
fn preserves_observation_order_while_expanding_erc1155_batch() {
    let caller = Address::repeat_byte(0x01);
    let native_target = Address::repeat_byte(0x02);
    let erc1155 = Address::repeat_byte(0x03);
    let erc721 = Address::repeat_byte(0x04);
    let operator = Address::repeat_byte(0x05);
    let from = Address::repeat_byte(0x06);
    let to = Address::repeat_byte(0x07);

    let candidates = collect_candidates(&[
        call_observation(caller, native_target, 1),
        erc1155_transfer_batch_observation(erc1155, operator, from, to, &[7, 8], &[9, 10]),
        erc721_transfer_observation(erc721, from, to, 42),
    ])
    .expect("ordered movement candidates");

    assert_eq!(
        candidates,
        vec![
            candidate(
                0,
                0,
                ChangeCandidateKind::NativeTransfer {
                    from: caller,
                    to: native_target,
                    amount: U256::from(1_u64),
                },
            ),
            candidate(
                1,
                0,
                ChangeCandidateKind::Erc1155Transfer {
                    collection: erc1155,
                    from,
                    to,
                    token_id: U256::from(7_u64),
                    amount: U256::from(9_u64),
                },
            ),
            candidate(
                1,
                1,
                ChangeCandidateKind::Erc1155Transfer {
                    collection: erc1155,
                    from,
                    to,
                    token_id: U256::from(8_u64),
                    amount: U256::from(10_u64),
                },
            ),
            candidate(
                2,
                0,
                ChangeCandidateKind::Erc721Transfer {
                    collection: erc721,
                    from,
                    to,
                    token_id: U256::from(42_u64),
                },
            ),
        ]
    );
}

#[test]
fn keeps_zero_amount_erc20_and_erc1155_movements_as_candidates() {
    let erc20 = Address::repeat_byte(0x01);
    let erc1155 = Address::repeat_byte(0x02);
    let operator = Address::repeat_byte(0x03);
    let from = Address::repeat_byte(0x04);
    let to = Address::repeat_byte(0x05);

    let candidates = collect_candidates(&[
        erc20_transfer_observation(erc20, from, to, 0),
        erc1155_transfer_single_observation(erc1155, operator, from, to, 7, 0),
    ])
    .expect("zero-amount movement candidates");

    assert_eq!(
        candidates,
        vec![
            candidate(
                0,
                0,
                ChangeCandidateKind::Erc20Transfer {
                    token: erc20,
                    from,
                    to,
                    amount: U256::ZERO,
                },
            ),
            candidate(
                1,
                0,
                ChangeCandidateKind::Erc1155Transfer {
                    collection: erc1155,
                    from,
                    to,
                    token_id: U256::from(7_u64),
                    amount: U256::ZERO,
                },
            ),
        ]
    );
}

#[test]
fn collects_authorization_candidates_and_normalizes_zero_erc721_approval() {
    let erc20 = Address::repeat_byte(0x01);
    let erc721 = Address::repeat_byte(0x02);
    let owner = Address::repeat_byte(0x03);
    let spender = Address::repeat_byte(0x04);
    let operator = Address::repeat_byte(0x05);

    let candidates = collect_candidates(&[
        erc20_approval_observation(erc20, owner, spender, 7),
        erc721_approval_observation(erc721, owner, Address::ZERO, 42),
        approval_for_all_observation(erc721, owner, operator, U256::from(1_u64)),
    ])
    .expect("authorization candidates");

    assert_eq!(
        candidates,
        vec![
            candidate(
                0,
                0,
                ChangeCandidateKind::Erc20Allowance {
                    token: erc20,
                    owner,
                    spender,
                    evidence: Erc20AllowanceEvidence::ApprovalEvent {
                        value: U256::from(7_u64),
                    },
                },
            ),
            candidate(
                1,
                0,
                ChangeCandidateKind::Erc721Approval {
                    collection: erc721,
                    owner,
                    approved_address: None,
                    token_id: U256::from(42_u64),
                },
            ),
            candidate(
                2,
                0,
                ChangeCandidateKind::OperatorApproval {
                    collection: erc721,
                    owner,
                    operator,
                    approved: true,
                },
            ),
        ]
    );
}

#[test]
fn requires_same_target_erc20_transfer_evidence_for_transfer_from_candidate() {
    let token = Address::repeat_byte(0x01);
    let other_target = Address::repeat_byte(0x02);
    let owner = Address::repeat_byte(0x03);
    let recipient = Address::repeat_byte(0x04);
    let spender = Address::repeat_byte(0x05);

    let candidates = collect_candidates(&[
        transfer_from_call_observation(spender, token, owner, recipient, 9),
        transfer_from_call_observation(spender, other_target, owner, recipient, 9),
        erc20_transfer_observation(token, owner, recipient, 9),
    ])
    .expect("transferFrom candidates");

    assert_eq!(
        candidates,
        vec![
            candidate(
                0,
                0,
                ChangeCandidateKind::Erc20Allowance {
                    token,
                    owner,
                    spender,
                    evidence: Erc20AllowanceEvidence::TransferFromCall {
                        amount: U256::from(9_u64),
                    },
                },
            ),
            candidate(
                2,
                0,
                ChangeCandidateKind::Erc20Transfer {
                    token,
                    from: owner,
                    to: recipient,
                    amount: U256::from(9_u64),
                },
            ),
        ]
    );
}

#[test]
fn ignores_unrelated_logs_but_rejects_malformed_supported_events() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let operator = Address::repeat_byte(0x03);
    let unrelated = Observation::Log {
        address: collection,
        topics: vec![B256::repeat_byte(0xff)],
        data: Bytes::from_static(b"unrelated"),
    };

    assert!(
        collect_candidates(std::slice::from_ref(&unrelated))
            .expect("unrelated log")
            .is_empty()
    );

    let malformed = approval_for_all_observation(collection, owner, operator, U256::from(2_u64));
    let error = collect_candidates(&[unrelated, malformed]).expect_err("malformed event");

    assert!(matches!(
        error,
        TransactionChangesError::MalformedEvent {
            observation_index: 1,
            source,
        } if source.event == SupportedEvent::ApprovalForAll
    ));
}

#[test]
fn rejects_nonzero_selfdestruct_to_self() {
    let contract = Address::repeat_byte(0x01);
    let amount = U256::from(7_u64);
    let error = collect_candidates(&[Observation::SelfDestruct {
        contract,
        target: contract,
        amount,
    }])
    .expect_err("selfdestruct-to-self");

    assert!(matches!(
        error,
        TransactionChangesError::UnsupportedSelfDestructToSelf {
            observation_index: 0,
            contract: error_contract,
            amount: error_amount,
        } if error_contract == contract && error_amount == amount
    ));
}

#[test]
fn reconciles_revm_native_transfer_and_fee_only_state() {
    const GAS_LIMIT: u64 = 21_000;
    const GAS_PRICE: u128 = 10;
    const BASE_FEE: u64 = 3;

    let caller = Address::repeat_byte(0x01);
    let receiver = Address::repeat_byte(0x02);
    let beneficiary = Address::repeat_byte(0x03);

    let mut db = InMemoryDB::default();
    db.insert_account_info(
        caller,
        AccountInfo::default().with_balance(U256::from(1_000_000_u64)),
    );
    db.insert_account_info(
        receiver,
        AccountInfo::default().with_balance(U256::from(10_u64)),
    );
    db.insert_account_info(
        beneficiary,
        AccountInfo::default().with_balance(U256::from(5_u64)),
    );

    let mut evm = Context::mainnet()
        .with_db(db)
        .modify_block_chained(|block| {
            block.basefee = BASE_FEE;
            block.beneficiary = beneficiary;
        })
        .build_mainnet_with_inspector(ChangeObservationInspector::new());
    let result_and_state = evm
        .inspect_tx(
            TxEnv::builder()
                .caller(caller)
                .kind(TxKind::Call(receiver))
                .value(U256::from(200_u64))
                .gas_limit(GAS_LIMIT)
                .gas_price(GAS_PRICE)
                .build()
                .expect("valid native transfer transaction"),
        )
        .expect("native transfer execution");
    let observations = std::mem::take(&mut evm.inspector).into_observations();
    let candidates = collect_candidates(&observations).expect("native transfer candidate");
    let gas = result_and_state.result.gas();
    let gas_precharge = U256::from(gas.limit()) * U256::from(GAS_PRICE);
    let fee = U256::from(gas.used()) * U256::from(GAS_PRICE);
    let caller_refund = gas_precharge - fee;
    let beneficiary_reward = U256::from(gas.used()) * U256::from(GAS_PRICE - u128::from(BASE_FEE));

    check_native_balances(
        &result_and_state.state,
        &candidates,
        caller,
        beneficiary,
        gas_precharge,
        caller_refund,
        beneficiary_reward,
    )
    .expect("Revm native balances should reconcile");

    let fee_only_state = native_state([
        (caller, U256::from(1_000_u64), U256::from(940_u64)),
        (beneficiary, U256::from(5_u64), U256::from(55_u64)),
    ]);

    check_native_balances(
        &fee_only_state,
        &[],
        caller,
        beneficiary,
        U256::from(100_u64),
        U256::from(40_u64),
        U256::from(50_u64),
    )
    .expect("fee-only balances should reconcile");
}

#[test]
fn rejects_balance_received_after_selfdestruct() {
    let caller = Address::repeat_byte(0x01);
    let destroyed = Address::repeat_byte(0x02);
    let target = Address::repeat_byte(0x03);

    let mut state = native_state([
        (caller, U256::from(25_u64), U256::ZERO),
        (destroyed, U256::from(300_u64), U256::from(25_u64)),
        (target, U256::ZERO, U256::from(300_u64)),
    ]);
    state
        .get_mut(&destroyed)
        .expect("destroyed account")
        .mark_selfdestruct();
    let candidates = [
        native_candidate(0, destroyed, target, U256::from(300_u64)),
        native_candidate(1, caller, destroyed, U256::from(25_u64)),
    ];

    let error = check_native_balances(
        &state,
        &candidates,
        caller,
        target,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("balance deleted at commit must not be reported as a transfer");

    assert!(matches!(
        error,
        TransactionChangesError::NativeBalanceMismatch {
            address,
            replayed_balance,
            state_balance,
        } if address == destroyed
            && replayed_balance == U256::from(25_u64)
            && state_balance == U256::ZERO
    ));
}

#[test]
fn rejects_invalid_native_balance_replay() {
    let caller = Address::repeat_byte(0x01);
    let target = Address::repeat_byte(0x02);
    let unrelated = Address::repeat_byte(0x03);

    let missing_state = native_state([(caller, U256::from(10_u64), U256::from(9_u64))]);
    let missing_error = check_native_balances(
        &missing_state,
        &[native_candidate(0, caller, target, U256::from(1_u64))],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("missing target account must fail");
    assert!(matches!(
        missing_error,
        TransactionChangesError::NativeAccountMissing { address } if address == target
    ));

    let underflow_state = native_state([(caller, U256::from(10_u64), U256::from(10_u64))]);
    let underflow_error = check_native_balances(
        &underflow_state,
        &[native_candidate(0, caller, caller, U256::from(20_u64))],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("underfunded self-transfer must fail");
    assert!(matches!(
        underflow_error,
        TransactionChangesError::NativeBalanceUnderflow { address, .. }
            if address == caller
    ));

    let unexplained_state = native_state([
        (caller, U256::from(10_u64), U256::from(10_u64)),
        (unrelated, U256::from(5_u64), U256::from(6_u64)),
    ]);
    let mismatch_error = check_native_balances(
        &unexplained_state,
        &[],
        caller,
        caller,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
    )
    .expect_err("unexplained state balance change must fail");
    assert!(matches!(
        mismatch_error,
        TransactionChangesError::NativeBalanceMismatch {
            address,
            replayed_balance,
            state_balance,
        } if address == unrelated
            && replayed_balance == U256::from(5_u64)
            && state_balance == U256::from(6_u64)
    ));
}

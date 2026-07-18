use std::collections::HashMap;

use alloy::sol_types::SolValue;
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};

use crate::{
    ApprovalChange, ApprovalForAllChange, Asset, BurnChange, Change, Collection, Erc20AssetDisplay,
    Erc721CollectionDisplay, MintChange, TransferChange, change_observation::Observation,
};

use super::{
    candidate::{
        ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence, ObservationPosition,
    },
    collection::collect_candidates,
    current_changes::build_current_changes,
    current_facts::{
        ContractKind, CurrentChangeFacts, CurrentFactRequests, Erc20Metadata,
        Erc721CollectionMetadata, Erc721CollectionMetadataRequest, derive_current_fact_requests,
    },
    error::TransactionChangesError,
    event_codec::SupportedEvent,
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
fn derives_deduplicated_current_fact_requests_in_first_candidate_order() {
    let operator_only = Address::repeat_byte(0x01);
    let operator_then_erc721 = Address::repeat_byte(0x02);
    let erc20 = Address::repeat_byte(0x03);
    let transfer_from_only = Address::repeat_byte(0x04);
    let owner = Address::repeat_byte(0x05);
    let operator = Address::repeat_byte(0x06);
    let recipient = Address::repeat_byte(0x07);

    let requests = derive_current_fact_requests(&[
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
        CurrentFactRequests {
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
fn builds_current_movement_changes_with_existing_public_semantics() {
    let native_from = Address::repeat_byte(0x01);
    let native_to = Address::repeat_byte(0x02);
    let erc20 = Address::repeat_byte(0x03);
    let erc721 = Address::repeat_byte(0x04);
    let erc1155 = Address::repeat_byte(0x05);
    let owner = Address::repeat_byte(0x06);
    let recipient = Address::repeat_byte(0x07);
    let operator = Address::repeat_byte(0x08);

    let candidates = collect_candidates(&[
        call_observation(native_from, native_to, 1),
        erc20_transfer_observation(erc20, owner, recipient, 2),
        erc721_transfer_observation(erc721, Address::ZERO, recipient, 42),
        erc1155_transfer_single_observation(erc1155, operator, owner, Address::ZERO, 7, 3),
    ])
    .expect("movement candidates");
    let facts = CurrentChangeFacts::new(
        HashMap::new(),
        HashMap::from([(
            erc20,
            Erc20Metadata {
                name: Some("USD Coin".to_string()),
                symbol: Some("USDC".to_string()),
                decimals: Some(6),
            },
        )]),
        HashMap::from([(
            erc721,
            Erc721CollectionMetadata {
                name: Some("Mock NFT Collection".to_string()),
                symbol: Some("MNFT".to_string()),
            },
        )]),
    );

    let changes = build_current_changes(candidates, &facts);

    assert_eq!(
        changes,
        vec![
            Change::Transfer(TransferChange {
                asset: Asset::Native { display: None },
                from: native_from,
                to: native_to,
                amount: Some(U256::from(1_u64)),
            }),
            Change::Transfer(TransferChange {
                asset: Asset::Erc20 {
                    contract_address: erc20,
                    display: Some(Erc20AssetDisplay {
                        name: Some("USD Coin".to_string()),
                        symbol: Some("USDC".to_string()),
                        decimals: Some(6),
                    }),
                },
                from: owner,
                to: recipient,
                amount: Some(U256::from(2_u64)),
            }),
            Change::Mint(MintChange {
                asset: Asset::Erc721 {
                    contract_address: erc721,
                    token_id: U256::from(42_u64),
                    collection: Some(Erc721CollectionDisplay {
                        name: Some("Mock NFT Collection".to_string()),
                        symbol: Some("MNFT".to_string()),
                    }),
                    token: None,
                },
                to: recipient,
                amount: None,
            }),
            Change::Burn(BurnChange {
                asset: Asset::Erc1155 {
                    contract_address: erc1155,
                    token_id: U256::from(7_u64),
                    collection: None,
                    token: None,
                },
                from: owner,
                amount: Some(U256::from(3_u64)),
            }),
        ]
    );
}

#[test]
fn builds_current_authorizations_without_emitting_unverified_transfer_from() {
    let erc20 = Address::repeat_byte(0x01);
    let erc721 = Address::repeat_byte(0x02);
    let erc1155 = Address::repeat_byte(0x03);
    let unknown = Address::repeat_byte(0x04);
    let owner = Address::repeat_byte(0x05);
    let spender = Address::repeat_byte(0x06);
    let operator = Address::repeat_byte(0x07);

    let candidates = vec![
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
            ChangeCandidateKind::Erc20Allowance {
                token: erc20,
                owner,
                spender,
                evidence: Erc20AllowanceEvidence::TransferFromCall {
                    amount: U256::from(2_u64),
                },
            },
        ),
        candidate(
            2,
            0,
            ChangeCandidateKind::Erc721Approval {
                collection: erc721,
                owner,
                approved_address: None,
                token_id: U256::from(42_u64),
            },
        ),
        candidate(
            3,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: erc721,
                owner,
                operator,
                approved: true,
            },
        ),
        candidate(
            4,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: erc1155,
                owner,
                operator,
                approved: false,
            },
        ),
        candidate(
            5,
            0,
            ChangeCandidateKind::OperatorApproval {
                collection: unknown,
                owner,
                operator,
                approved: true,
            },
        ),
    ];
    let facts = CurrentChangeFacts::new(
        HashMap::from([
            (erc721, ContractKind::Erc721),
            (erc1155, ContractKind::Erc1155),
        ]),
        HashMap::from([(
            erc20,
            Erc20Metadata {
                name: Some("USD Coin".to_string()),
                symbol: Some("USDC".to_string()),
                decimals: Some(6),
            },
        )]),
        HashMap::from([(
            erc721,
            Erc721CollectionMetadata {
                name: Some("Mock NFT Collection".to_string()),
                symbol: Some("MNFT".to_string()),
            },
        )]),
    );

    let changes = build_current_changes(candidates, &facts);

    assert_eq!(
        changes,
        vec![
            Change::Approval(ApprovalChange {
                asset: Asset::Erc20 {
                    contract_address: erc20,
                    display: Some(Erc20AssetDisplay {
                        name: Some("USD Coin".to_string()),
                        symbol: Some("USDC".to_string()),
                        decimals: Some(6),
                    }),
                },
                owner,
                spender,
                amount: Some(U256::from(7_u64)),
            }),
            Change::Approval(ApprovalChange {
                asset: Asset::Erc721 {
                    contract_address: erc721,
                    token_id: U256::from(42_u64),
                    collection: Some(Erc721CollectionDisplay {
                        name: Some("Mock NFT Collection".to_string()),
                        symbol: Some("MNFT".to_string()),
                    }),
                    token: None,
                },
                owner,
                spender: Address::ZERO,
                amount: None,
            }),
            Change::ApprovalForAll(ApprovalForAllChange {
                collection: Collection::Erc721 {
                    contract_address: erc721,
                    collection: Some(Erc721CollectionDisplay {
                        name: Some("Mock NFT Collection".to_string()),
                        symbol: Some("MNFT".to_string()),
                    }),
                },
                owner,
                operator,
                approved: true,
            }),
            Change::ApprovalForAll(ApprovalForAllChange {
                collection: Collection::Erc1155 {
                    contract_address: erc1155,
                    collection: None,
                },
                owner,
                operator,
                approved: false,
            }),
        ]
    );
}

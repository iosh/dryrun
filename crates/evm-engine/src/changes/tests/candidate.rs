use alloy::sol_types::SolValue;
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};

use super::{
    super::{
        candidate::{ChangeCandidateKind, Erc20AllowanceEvidence, collect_candidates},
        error::TransactionChangesError,
        event_codec::SupportedEvent,
        observation::Observation,
    },
    support::{candidate, erc20_movement_candidate, native_candidate},
};

fn indexed_address(address: Address) -> B256 {
    address.into_word()
}

fn indexed_u256(value: U256) -> B256 {
    B256::from(value.to_be_bytes::<32>())
}

fn event_topic(signature: &str) -> B256 {
    keccak256(signature)
}

fn event_observation<const N: usize>(
    address: Address,
    signature: &str,
    indexed_topics: [B256; N],
    data: Bytes,
) -> Observation {
    let mut topics = Vec::with_capacity(N + 1);
    topics.push(event_topic(signature));
    topics.extend(indexed_topics);

    Observation::Log {
        address,
        topics,
        data,
    }
}

fn uint_data(value: u64) -> Bytes {
    Bytes::from(U256::from(value).to_be_bytes_vec())
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
    event_observation(
        token,
        "Transfer(address,address,uint256)",
        [indexed_address(from), indexed_address(to)],
        uint_data(amount),
    )
}

fn weth9_event_observation(
    token: Address,
    signature: &str,
    account: Address,
    amount: u64,
) -> Observation {
    event_observation(
        token,
        signature,
        [indexed_address(account)],
        uint_data(amount),
    )
}

fn erc721_transfer_observation(
    collection: Address,
    from: Address,
    to: Address,
    token_id: u64,
) -> Observation {
    event_observation(
        collection,
        "Transfer(address,address,uint256)",
        [
            indexed_address(from),
            indexed_address(to),
            indexed_u256(U256::from(token_id)),
        ],
        Bytes::new(),
    )
}

fn erc20_approval_observation(
    token: Address,
    owner: Address,
    spender: Address,
    value: u64,
) -> Observation {
    event_observation(
        token,
        "Approval(address,address,uint256)",
        [indexed_address(owner), indexed_address(spender)],
        uint_data(value),
    )
}

fn erc721_approval_observation(
    collection: Address,
    owner: Address,
    approved_address: Address,
    token_id: u64,
) -> Observation {
    event_observation(
        collection,
        "Approval(address,address,uint256)",
        [
            indexed_address(owner),
            indexed_address(approved_address),
            indexed_u256(U256::from(token_id)),
        ],
        Bytes::new(),
    )
}

fn approval_for_all_observation(
    collection: Address,
    owner: Address,
    operator: Address,
    approved_word: U256,
) -> Observation {
    event_observation(
        collection,
        "ApprovalForAll(address,address,bool)",
        [indexed_address(owner), indexed_address(operator)],
        Bytes::from(approved_word.to_be_bytes_vec()),
    )
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
    event_observation(
        collection,
        "TransferSingle(address,address,address,uint256,uint256)",
        [
            indexed_address(operator),
            indexed_address(from),
            indexed_address(to),
        ],
        Bytes::from((U256::from(token_id), U256::from(amount)).abi_encode_sequence()),
    )
}

fn erc1155_transfer_batch_observation(
    collection: Address,
    operator: Address,
    from: Address,
    to: Address,
    token_ids: &[u64],
    amounts: &[u64],
) -> Observation {
    event_observation(
        collection,
        "TransferBatch(address,address,address,uint256[],uint256[])",
        [
            indexed_address(operator),
            indexed_address(from),
            indexed_address(to),
        ],
        Bytes::from(
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
    )
}

#[test]
fn collects_only_value_backed_weth9_movements() {
    let weth = Address::repeat_byte(0x01);
    let account = Address::repeat_byte(0x02);
    let candidates = collect_candidates(&[
        call_observation(account, weth, 5),
        weth9_event_observation(weth, "Deposit(address,uint256)", account, 5),
        call_observation(weth, account, 2),
        weth9_event_observation(weth, "Withdrawal(address,uint256)", account, 2),
        weth9_event_observation(weth, "Deposit(address,uint256)", account, 5),
    ])
    .expect("WETH9 candidates");

    assert_eq!(
        candidates,
        vec![
            native_candidate(0, account, weth, U256::from(5_u64)),
            erc20_movement_candidate(1, weth, Address::ZERO, account, U256::from(5_u64)),
            native_candidate(2, weth, account, U256::from(2_u64)),
            erc20_movement_candidate(3, weth, account, Address::ZERO, U256::from(2_u64)),
        ]
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
fn keeps_zero_amount_movements_as_candidates() {
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
                ChangeCandidateKind::Erc20Movement {
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
fn collects_authorizations_and_normalizes_zero_erc721_approval() {
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
fn requires_matching_transfer_evidence_for_transfer_from() {
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
                ChangeCandidateKind::Erc20Movement {
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

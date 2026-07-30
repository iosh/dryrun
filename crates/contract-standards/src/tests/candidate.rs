use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use alloy_sol_types::SolValue;

use crate::{
    ContractStandardsError, EventCodecError, Position, Record, SupportedEvent,
    candidate::{AllowanceSource, StandardCandidateKind},
    collect_candidates,
};

fn topic(address: Address) -> B256 {
    address.into_word()
}

fn log<const N: usize>(
    index: usize,
    address: Address,
    signature: &str,
    indexed: [B256; N],
    data: Bytes,
) -> Record {
    let mut topics = vec![keccak256(signature)];
    topics.extend(indexed);

    Record::Log {
        position: Position::new(index, 0),
        address,
        topics,
        data,
    }
}

fn transfer_from(
    index: usize,
    caller: Address,
    token: Address,
    owner: Address,
    recipient: Address,
    amount: u64,
) -> Record {
    let mut input = Vec::with_capacity(100);
    input.extend_from_slice(&keccak256("transferFrom(address,address,uint256)")[..4]);
    input.extend_from_slice(topic(owner).as_slice());
    input.extend_from_slice(topic(recipient).as_slice());
    input.extend_from_slice(&U256::from(amount).to_be_bytes::<32>());

    Record::Call {
        position: Position::new(index, 0),
        caller,
        target: token,
        value: U256::ZERO,
        input_len: input.len(),
        input_prefix: input.into(),
    }
}

#[test]
fn parses_transfers() {
    let erc20 = Address::repeat_byte(0x01);
    let erc721 = Address::repeat_byte(0x02);
    let erc1155 = Address::repeat_byte(0x03);
    let from = Address::repeat_byte(0x04);
    let to = Address::repeat_byte(0x05);
    let operator = Address::repeat_byte(0x06);
    let records = [
        log(
            0,
            erc20,
            "Transfer(address,address,uint256)",
            [topic(from), topic(to)],
            U256::ZERO.to_be_bytes_vec().into(),
        ),
        log(
            1,
            erc721,
            "Transfer(address,address,uint256)",
            [topic(from), topic(to), B256::from(U256::from(7_u64))],
            Bytes::new(),
        ),
        log(
            2,
            erc1155,
            "TransferBatch(address,address,address,uint256[],uint256[])",
            [topic(operator), topic(from), topic(to)],
            Bytes::from(
                (
                    vec![U256::from(8_u64), U256::from(9_u64)],
                    vec![U256::from(10_u64), U256::from(11_u64)],
                )
                    .abi_encode_sequence(),
            ),
        ),
    ];

    let candidates = collect_candidates(&records).expect("transfers");

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.position())
            .collect::<Vec<_>>(),
        vec![
            Position::new(0, 0),
            Position::new(1, 0),
            Position::new(2, 0),
            Position::new(2, 1),
        ]
    );
    assert!(matches!(
        candidates[0].kind,
        StandardCandidateKind::Erc20Movement { amount, .. } if amount.is_zero()
    ));
    assert!(matches!(
        candidates[1].kind,
        StandardCandidateKind::Erc721Transfer { token_id, .. }
            if token_id == U256::from(7_u64)
    ));
    assert!(matches!(
        candidates[3].kind,
        StandardCandidateKind::Erc1155Transfer { token_id, amount, .. }
            if token_id == U256::from(9_u64) && amount == U256::from(11_u64)
    ));
}

#[test]
fn parses_approvals() {
    let token = Address::repeat_byte(0x01);
    let collection = Address::repeat_byte(0x02);
    let owner = Address::repeat_byte(0x03);
    let recipient = Address::repeat_byte(0x04);
    let spender = Address::repeat_byte(0x05);
    let records = [
        transfer_from(0, spender, token, owner, recipient, 6),
        log(
            1,
            token,
            "Transfer(address,address,uint256)",
            [topic(owner), topic(recipient)],
            U256::from(6_u64).to_be_bytes_vec().into(),
        ),
        log(
            2,
            token,
            "Approval(address,address,uint256)",
            [topic(owner), topic(spender)],
            U256::from(7_u64).to_be_bytes_vec().into(),
        ),
        log(
            3,
            collection,
            "Approval(address,address,uint256)",
            [
                topic(owner),
                topic(Address::ZERO),
                B256::from(U256::from(8_u64)),
            ],
            Bytes::new(),
        ),
        log(
            4,
            collection,
            "ApprovalForAll(address,address,bool)",
            [topic(owner), topic(spender)],
            U256::from(1_u64).to_be_bytes_vec().into(),
        ),
    ];

    let candidates = collect_candidates(&records).expect("approvals");

    assert!(matches!(
        candidates[0].kind,
        StandardCandidateKind::Erc20Allowance {
            source: AllowanceSource::TransferFromCall { amount },
            ..
        } if amount == U256::from(6_u64)
    ));
    assert!(matches!(
        candidates[2].kind,
        StandardCandidateKind::Erc20Allowance {
            source: AllowanceSource::ApprovalEvent { value },
            ..
        } if value == U256::from(7_u64)
    ));
    assert!(matches!(
        candidates[3].kind,
        StandardCandidateKind::Erc721Approval {
            approved_address: None,
            ..
        }
    ));
    assert!(matches!(
        candidates[4].kind,
        StandardCandidateKind::OperatorApproval { approved: true, .. }
    ));
}

#[test]
fn rejects_malformed_event() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let operator = Address::repeat_byte(0x03);
    let malformed = log(
        1,
        collection,
        "ApprovalForAll(address,address,bool)",
        [topic(owner), topic(operator)],
        U256::from(2_u64).to_be_bytes_vec().into(),
    );

    assert_eq!(
        collect_candidates(&[malformed]),
        Err(ContractStandardsError::MalformedEvent {
            position: Position::new(1, 0),
            source: EventCodecError::malformed(
                SupportedEvent::ApprovalForAll,
                "approved value is not a canonical bool",
            ),
        })
    );
}

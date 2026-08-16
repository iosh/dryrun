use alloy_primitives::{Address, B256, U256, keccak256};
use alloy_sol_types::{SolCall, SolValue, sol};
use contract_standards::{
    DecodedStandardLog, Erc20Metadata, Erc721CollectionMetadata, Erc1155TransferItem, MetadataCall,
    MetadataValues, MissingMetadataOutcome, StandardChange, decode_standard_log, metadata_calls,
};

sol! {
    contract TestMetadata {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
}

const TRANSFER: &str = "Transfer(address,address,uint256)";
const APPROVAL: &str = "Approval(address,address,uint256)";
const APPROVAL_FOR_ALL: &str = "ApprovalForAll(address,address,bool)";
const TRANSFER_SINGLE: &str = "TransferSingle(address,address,address,uint256,uint256)";
const TRANSFER_BATCH: &str = "TransferBatch(address,address,address,uint256[],uint256[])";

fn address(byte: u8) -> Address {
    Address::repeat_byte(byte)
}

fn address_topic(value: Address) -> B256 {
    value.into_word()
}

fn value_topic(value: U256) -> B256 {
    B256::from(value.to_be_bytes::<32>())
}

fn value_data(value: U256) -> Vec<u8> {
    value.to_be_bytes::<32>().to_vec()
}

fn topics(signature: &str, indexed: &[B256]) -> Vec<B256> {
    std::iter::once(keccak256(signature))
        .chain(indexed.iter().copied())
        .collect()
}

fn standard_log(
    contract: Address,
    signature: &str,
    indexed: &[B256],
    data: &[u8],
) -> DecodedStandardLog<Address> {
    decode_standard_log(contract, &topics(signature, indexed), data, |value| value)
        .expect("standard log should decode")
}

fn data_word_log(
    contract: Address,
    signature: &str,
    first: Address,
    second: Address,
    value: U256,
) -> DecodedStandardLog<Address> {
    standard_log(
        contract,
        signature,
        &[address_topic(first), address_topic(second)],
        &value_data(value),
    )
}

fn indexed_word_log(
    contract: Address,
    signature: &str,
    first: Address,
    second: Address,
    value: U256,
) -> DecodedStandardLog<Address> {
    standard_log(
        contract,
        signature,
        &[
            address_topic(first),
            address_topic(second),
            value_topic(value),
        ],
        &[],
    )
}

fn metadata_calls_for(contract: Address) -> [MetadataCall<Address>; 3] {
    [
        MetadataCall::Name {
            contract_address: contract,
        },
        MetadataCall::Symbol {
            contract_address: contract,
        },
        MetadataCall::Decimals {
            contract_address: contract,
        },
    ]
}

fn unavailable_metadata(contracts: &[Address]) -> MetadataValues<Address> {
    let mut values = MetadataValues::default();
    for contract in contracts {
        for call in metadata_calls_for(*contract) {
            values.record_unavailable(call);
        }
    }
    values
}

fn batch_data(token_ids: &[U256], amounts: &[U256]) -> Vec<u8> {
    (token_ids, amounts).abi_encode_sequence()
}

fn change_kind(change: &StandardChange<Address>) -> &'static str {
    match change {
        StandardChange::Erc20Transfer { .. } => "erc20-transfer",
        StandardChange::Erc20Approval { .. } => "erc20-approval",
        StandardChange::Erc721Transfer { .. } => "erc721-transfer",
        StandardChange::Erc721Approval { .. } => "erc721-approval",
        StandardChange::OperatorApproval { .. } => "operator-approval",
        StandardChange::Erc1155TransferSingle { .. } => "erc1155-single",
        StandardChange::Erc1155TransferBatch { .. } => "erc1155-batch",
    }
}

#[test]
fn decodes_canonical_occurrences() {
    let contract = address(1);
    let from = address(2);
    let to = address(3);
    let owner = address(4);
    let other = address(5);
    let token_ids = [U256::from(11), U256::from(11), U256::from(12)];
    let amounts = [U256::from(3), U256::ZERO, U256::from(4)];
    let logs = vec![
        data_word_log(contract, TRANSFER, Address::ZERO, Address::ZERO, U256::ZERO),
        data_word_log(contract, APPROVAL, owner, other, U256::from(7)),
        indexed_word_log(contract, TRANSFER, from, to, U256::from(8)),
        indexed_word_log(contract, APPROVAL, owner, Address::ZERO, U256::from(9)),
        data_word_log(contract, APPROVAL_FOR_ALL, owner, other, U256::ZERO),
        standard_log(
            contract,
            TRANSFER_SINGLE,
            &[address_topic(owner), address_topic(from), address_topic(to)],
            &(U256::from(10), U256::from(2)).abi_encode_sequence(),
        ),
        standard_log(
            contract,
            TRANSFER_BATCH,
            &[address_topic(owner), address_topic(from), address_topic(to)],
            &batch_data(&token_ids, &amounts),
        ),
    ];
    let operator_log = logs[4].clone();
    let metadata = unavailable_metadata(&[contract]);
    let changes = logs
        .into_iter()
        .map(|log| log.into_change(&metadata))
        .collect::<Result<Vec<_>, _>>()
        .expect("metadata outcomes are complete");

    assert_eq!(
        changes.iter().map(change_kind).collect::<Vec<_>>(),
        [
            "erc20-transfer",
            "erc20-approval",
            "erc721-transfer",
            "erc721-approval",
            "operator-approval",
            "erc1155-single",
            "erc1155-batch",
        ]
    );
    assert!(matches!(
        &changes[0],
        StandardChange::Erc20Transfer {
            from: Address::ZERO,
            to: Address::ZERO,
            raw_amount: U256::ZERO,
            ..
        }
    ));
    assert!(matches!(
        &changes[3],
        StandardChange::Erc721Approval {
            approved_address: None,
            ..
        }
    ));
    assert_eq!(
        match &changes[6] {
            StandardChange::Erc1155TransferBatch { items, .. } => items,
            _ => unreachable!(),
        }
        .as_slice(),
        &[
            Erc1155TransferItem {
                token_id: token_ids[0],
                raw_amount: amounts[0],
            },
            Erc1155TransferItem {
                token_id: token_ids[1],
                raw_amount: amounts[1],
            },
            Erc1155TransferItem {
                token_id: token_ids[2],
                raw_amount: amounts[2],
            },
        ]
    );
    assert!(operator_log.into_change(&MetadataValues::default()).is_ok());
}

#[test]
fn preserves_occurrences_and_metadata_order() {
    let first_contract = address(1);
    let second_contract = address(2);
    let first = address(3);
    let second = address(4);
    let amount = U256::from(25);
    let logs = vec![
        indexed_word_log(first_contract, TRANSFER, first, second, U256::from(1)),
        data_word_log(second_contract, TRANSFER, first, second, amount),
        data_word_log(second_contract, TRANSFER, second, first, amount),
        data_word_log(first_contract, APPROVAL, first, second, U256::from(2)),
    ];

    assert_eq!(
        metadata_calls(&logs),
        vec![
            MetadataCall::Name {
                contract_address: first_contract,
            },
            MetadataCall::Symbol {
                contract_address: first_contract,
            },
            MetadataCall::Name {
                contract_address: second_contract,
            },
            MetadataCall::Symbol {
                contract_address: second_contract,
            },
            MetadataCall::Decimals {
                contract_address: second_contract,
            },
            MetadataCall::Decimals {
                contract_address: first_contract,
            },
        ]
    );

    let metadata = unavailable_metadata(&[first_contract, second_contract]);
    let directions = logs
        .into_iter()
        .filter_map(
            |log| match log.into_change(&metadata).expect("metadata is complete") {
                StandardChange::Erc20Transfer {
                    from,
                    to,
                    raw_amount,
                    ..
                } => Some((from, to, raw_amount)),
                _ => None,
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(
        directions,
        vec![(first, second, amount), (second, first, amount)]
    );
}

#[test]
fn ignores_unknown_and_malformed_logs() {
    let contract = address(1);
    let from = address(2);
    let to = address(3);
    let operator = address(4);
    let batch_topics = topics(
        TRANSFER_BATCH,
        &[
            address_topic(operator),
            address_topic(from),
            address_topic(to),
        ],
    );
    let mut non_canonical_batch = batch_data(&[U256::from(1)], &[U256::from(2)]);
    non_canonical_batch.extend_from_slice(&[0; 32]);

    let cases = vec![
        ("no topics", Vec::new(), Vec::new()),
        ("unknown topic", vec![B256::repeat_byte(0xff)], Vec::new()),
        ("wrong shape", vec![keccak256(TRANSFER)], Vec::new()),
        (
            "non-canonical address",
            topics(TRANSFER, &[B256::repeat_byte(1), address_topic(to)]),
            value_data(U256::from(1)),
        ),
        (
            "non-canonical bool",
            topics(
                APPROVAL_FOR_ALL,
                &[address_topic(from), address_topic(operator)],
            ),
            value_data(U256::from(2)),
        ),
        (
            "mismatched batch arrays",
            batch_topics.clone(),
            batch_data(&[U256::from(1)], &[U256::from(2), U256::from(3)]),
        ),
        (
            "non-canonical batch encoding",
            batch_topics,
            non_canonical_batch,
        ),
    ];

    for (name, topics, data) in cases {
        assert!(
            decode_standard_log(contract, &topics, &data, |value| value).is_none(),
            "{name}"
        );
    }
}

#[test]
fn metadata_codec_requires_recorded_outcomes() {
    let contract = address(1);
    for (call, selector) in metadata_calls_for(contract).into_iter().zip([
        [0x06, 0xfd, 0xde, 0x03],
        [0x95, 0xd8, 0x9b, 0x41],
        [0x31, 0x3c, 0xe5, 0x67],
    ]) {
        assert_eq!(call.call_data().as_ref(), selector.as_slice());
    }

    let name = "Token".to_owned();
    let mut values = MetadataValues::default();
    values.record_output(
        MetadataCall::Name {
            contract_address: contract,
        },
        &TestMetadata::nameCall::abi_encode_returns(&name),
    );
    values.record_output(
        MetadataCall::Symbol {
            contract_address: contract,
        },
        &TestMetadata::symbolCall::abi_encode_returns(&String::new()),
    );
    values.record_output(
        MetadataCall::Decimals {
            contract_address: contract,
        },
        &TestMetadata::decimalsCall::abi_encode_returns(&18),
    );
    assert_eq!(
        values.erc20_metadata(&contract),
        Ok(Erc20Metadata {
            name: Some(name),
            symbol: Some(String::new()),
            decimals: Some(18),
        })
    );

    let invalid_contract = address(2);
    for call in metadata_calls_for(invalid_contract) {
        values.record_output(call, &[0xff]);
    }
    assert_eq!(
        values.erc20_metadata(&invalid_contract),
        Ok(Erc20Metadata::default())
    );

    let erc20 = data_word_log(contract, TRANSFER, address(3), address(4), U256::from(1));
    let mut unavailable = MetadataValues::default();
    assert_eq!(
        erc20.clone().into_change(&unavailable),
        Err(MissingMetadataOutcome)
    );
    unavailable.record_unavailable(MetadataCall::Name {
        contract_address: contract,
    });
    unavailable.record_unavailable(MetadataCall::Symbol {
        contract_address: contract,
    });
    assert_eq!(
        erc20.clone().into_change(&unavailable),
        Err(MissingMetadataOutcome)
    );
    unavailable.record_unavailable(MetadataCall::Decimals {
        contract_address: contract,
    });
    assert!(matches!(
        erc20.into_change(&unavailable),
        Ok(StandardChange::Erc20Transfer { metadata, .. })
            if metadata == Erc20Metadata::default()
    ));

    let erc721 = indexed_word_log(contract, TRANSFER, address(3), address(4), U256::from(2));
    let mut unavailable = MetadataValues::default();
    unavailable.record_unavailable(MetadataCall::Name {
        contract_address: contract,
    });
    assert_eq!(
        erc721.clone().into_change(&unavailable),
        Err(MissingMetadataOutcome)
    );
    unavailable.record_unavailable(MetadataCall::Symbol {
        contract_address: contract,
    });
    assert!(matches!(
        erc721.into_change(&unavailable),
        Ok(StandardChange::Erc721Transfer { metadata, .. })
            if metadata == Erc721CollectionMetadata::default()
    ));
}

use std::collections::HashMap;

use alloy_primitives::{Address, U256};

use crate::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};

use super::super::{
    PositionedChange, build_changes,
    candidate::ObservationPosition,
    metadata::{ChangeMetadata, ChangeMetadataRequests, collect_change_metadata_requests},
    sort_changes_by_position,
};

fn positioned(observation_index: usize, change: Change) -> PositionedChange {
    PositionedChange::new(
        ObservationPosition {
            observation_index,
            item_index: 0,
        },
        change,
    )
}

#[test]
fn sorts_changes_before_metadata_collection_and_enrichment() {
    let erc721 = Address::repeat_byte(0x01);
    let second_erc721 = Address::repeat_byte(0x02);
    let erc20 = Address::repeat_byte(0x03);
    let owner = Address::repeat_byte(0x04);
    let recipient = Address::repeat_byte(0x05);
    let mut changes = vec![
        positioned(
            2,
            Change::Erc721OperatorApproval {
                contract_address: erc721,
                owner,
                operator: recipient,
                approved_before: false,
                approved_after: true,
                metadata: Erc721CollectionMetadata::default(),
            },
        ),
        positioned(
            0,
            Change::Erc20Transfer {
                contract_address: erc20,
                from: owner,
                to: recipient,
                raw_amount: U256::from(1_u64),
                metadata: Erc20Metadata::default(),
            },
        ),
        positioned(
            1,
            Change::Erc721Transfer {
                contract_address: second_erc721,
                from: owner,
                to: recipient,
                token_id: U256::from(2_u64),
                metadata: Erc721CollectionMetadata::default(),
            },
        ),
        positioned(
            3,
            Change::Erc20Allowance {
                contract_address: erc20,
                owner,
                spender: recipient,
                raw_amount_before: U256::ZERO,
                raw_amount_after: U256::from(3_u64),
                metadata: Erc20Metadata::default(),
            },
        ),
    ];

    sort_changes_by_position(&mut changes);
    assert_eq!(
        collect_change_metadata_requests(&changes),
        ChangeMetadataRequests {
            erc20_contracts: vec![erc20],
            erc721_collections: vec![second_erc721, erc721],
        }
    );

    let metadata = ChangeMetadata::new(
        NativeMetadata::default(),
        [(
            erc20,
            Erc20Metadata {
                name: None,
                symbol: Some("TOK".to_string()),
                decimals: Some(6),
            },
        )]
        .into_iter()
        .collect(),
        HashMap::new(),
    );
    let changes = build_changes(changes, &metadata);

    assert!(matches!(
        &changes[0],
        Change::Erc20Transfer {
            metadata: Erc20Metadata {
                symbol: Some(symbol),
                decimals: Some(6),
                ..
            },
            ..
        } if symbol == "TOK"
    ));
    assert!(matches!(changes[1], Change::Erc721Transfer { .. }));
    assert!(matches!(changes[2], Change::Erc721OperatorApproval { .. }));
}

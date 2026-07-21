use alloy_primitives::{Address, B256, U256};

use super::{
    super::{
        candidate::ChangeCandidateKind,
        error::TransactionChangesError,
        token_contract::check_token_contracts,
        token_state::{CollectionStandards, TokenStateValues, collect_token_state_keys},
    },
    support::candidate,
};

fn state_values(contract: Address, standards: CollectionStandards) -> TokenStateValues {
    let mut values = TokenStateValues::default();
    values
        .contract_code_hashes
        .insert(contract, B256::repeat_byte(0x11));
    values.collection_standards.insert(contract, standards);
    values
}

#[test]
fn requires_code_and_collection_standards_to_remain_stable() {
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
    let before = state_values(
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
    let supports_both = state_values(
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

    let erc1155_only = state_values(
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
        let values = state_values(collection, standards);
        assert_eq!(
            check_token_contracts(&operator_candidates, &operator_keys, &values, &values),
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
        let values = state_values(collection, standards);
        assert!(matches!(
            check_token_contracts(&operator_candidates, &operator_keys, &values, &values),
            Err(TransactionChangesError::OperatorApprovalStandardAmbiguous {
                collection: ambiguous_collection,
                ..
            }) if ambiguous_collection == collection
        ));
    }
}

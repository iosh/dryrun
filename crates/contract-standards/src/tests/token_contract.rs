use alloy_primitives::{Address, B256, U256};

use crate::{
    CollectionStandards, ContractStandardsError, StandardStateValues,
    candidate::StandardCandidateKind, state_requirements,
};

use super::{super::token_contract::check_token_contracts, support::candidate};

fn state(contract: Address, standards: CollectionStandards) -> StandardStateValues {
    let mut values = StandardStateValues::default();
    values
        .contract_code_hashes
        .insert(contract, B256::repeat_byte(0x11));
    values.collection_standards.insert(contract, standards);
    values
}

#[test]
fn checks_contract_lifecycle() {
    let collection = Address::repeat_byte(0x01);
    let owner = Address::repeat_byte(0x02);
    let recipient = Address::repeat_byte(0x03);
    let candidates = [candidate(
        0,
        0,
        StandardCandidateKind::Erc721Transfer {
            collection,
            from: owner,
            to: recipient,
            token_id: U256::from(1_u64),
        },
    )];
    let requirements = state_requirements(&candidates);
    let before = state(
        collection,
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: false,
        },
    );

    assert_eq!(
        check_token_contracts(&candidates, &requirements, &before, &before),
        Ok(())
    );

    let mut after = before.clone();
    after
        .contract_code_hashes
        .insert(collection, B256::repeat_byte(0x22));
    assert!(matches!(
        check_token_contracts(&candidates, &requirements, &before, &after),
        Err(ContractStandardsError::TokenContractCodeChanged { .. })
    ));

    let mut after = before.clone();
    after.collection_standards.insert(
        collection,
        CollectionStandards {
            supports_erc721: false,
            supports_erc1155: true,
        },
    );
    assert!(matches!(
        check_token_contracts(&candidates, &requirements, &before, &after),
        Err(ContractStandardsError::CollectionStandardsChanged { .. })
    ));

    assert!(matches!(
        check_token_contracts(&candidates, &requirements, &after, &after),
        Err(ContractStandardsError::CollectionStandardNotSupported {
            standard: "ERC-721",
            ..
        })
    ));

    let operator_candidates = [candidate(
        0,
        0,
        StandardCandidateKind::OperatorApproval {
            collection,
            owner,
            operator: recipient,
            approved: true,
        },
    )];
    let operator_requirements = state_requirements(&operator_candidates);
    let ambiguous = state(
        collection,
        CollectionStandards {
            supports_erc721: true,
            supports_erc1155: true,
        },
    );
    assert!(matches!(
        check_token_contracts(
            &operator_candidates,
            &operator_requirements,
            &ambiguous,
            &ambiguous,
        ),
        Err(ContractStandardsError::OperatorApprovalStandardAmbiguous { .. })
    ));
}

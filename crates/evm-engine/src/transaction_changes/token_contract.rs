//! Token contract lifecycle and standard checks.
use alloy_primitives::Address;

use super::{
    candidate::{ChangeCandidate, ChangeCandidateKind},
    error::TransactionChangesError,
    token_state::{CollectionStandards, TokenStateKeys, TokenStateValues},
};

pub(crate) fn check_token_contracts(
    candidates: &[ChangeCandidate],
    keys: &TokenStateKeys,
    before: &TokenStateValues,
    after: &TokenStateValues,
) -> Result<(), TransactionChangesError> {
    for &contract in &keys.token_contracts {
        let before_code_hash = before.contract_code_hashes.get(&contract).copied().ok_or(
            TransactionChangesError::TokenStateValueMissing {
                address: contract,
                value: "before runtime code hash",
            },
        )?;

        let after_code_hash = after.contract_code_hashes.get(&contract).copied().ok_or(
            TransactionChangesError::TokenStateValueMissing {
                address: contract,
                value: "after runtime code hash",
            },
        )?;

        if before_code_hash != after_code_hash {
            return Err(TransactionChangesError::TokenContractCodeChanged {
                contract,
                before_code_hash,
                after_code_hash,
            });
        }
    }

    for &collection in &keys.collection_standards {
        let before_standards =
            collection_standards(before, collection, "before collection standards")?;
        let after_standards =
            collection_standards(after, collection, "after collection standards")?;

        if before_standards != after_standards {
            return Err(TransactionChangesError::CollectionStandardsChanged {
                collection,
                before: before_standards,
                after: after_standards,
            });
        }
    }

    for candidate in candidates {
        match candidate.kind {
            ChangeCandidateKind::Erc721Transfer { collection, .. }
            | ChangeCandidateKind::Erc721Approval { collection, .. } => {
                let standards =
                    collection_standards(before, collection, "before collection standards")?;

                if !standards.supports_erc721 {
                    return Err(TransactionChangesError::CollectionStandardNotSupported {
                        collection,
                        standard: "ERC-721",
                    });
                }
            }

            ChangeCandidateKind::Erc1155Transfer { collection, .. } => {
                let standards =
                    collection_standards(before, collection, "before collection standards")?;

                if !standards.supports_erc1155 {
                    return Err(TransactionChangesError::CollectionStandardNotSupported {
                        collection,
                        standard: "ERC-1155",
                    });
                }
            }

            ChangeCandidateKind::OperatorApproval { collection, .. } => {
                let standards =
                    collection_standards(before, collection, "before collection standards")?;

                if standards.supports_erc721 == standards.supports_erc1155 {
                    return Err(TransactionChangesError::OperatorApprovalStandardAmbiguous {
                        collection,
                        supports_erc721: standards.supports_erc721,
                        supports_erc1155: standards.supports_erc1155,
                    });
                }
            }

            ChangeCandidateKind::NativeTransfer { .. }
            | ChangeCandidateKind::Erc20Transfer { .. }
            | ChangeCandidateKind::Erc20Allowance { .. } => {}
        }
    }

    Ok(())
}

fn collection_standards(
    values: &TokenStateValues,
    collection: Address,
    value: &'static str,
) -> Result<CollectionStandards, TransactionChangesError> {
    values.collection_standards.get(&collection).copied().ok_or(
        TransactionChangesError::TokenStateValueMissing {
            address: collection,
            value,
        },
    )
}

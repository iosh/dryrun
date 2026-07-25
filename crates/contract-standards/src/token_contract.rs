//! Token contract lifecycle and standard checks.
use alloy_primitives::Address;

use crate::{
    CollectionStandards, ContractStandardsError, StandardCandidate, StandardStateValues,
    StatePhase, StateRequirement, StateRequirements, candidate::StandardCandidateKind,
};

pub(crate) fn check_token_contracts(
    candidates: &[StandardCandidate],
    keys: &StateRequirements,
    before: &StandardStateValues,
    after: &StandardStateValues,
) -> Result<(), ContractStandardsError> {
    for &contract in &keys.token_contracts {
        let before_code_hash = before.contract_code_hashes.get(&contract).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::TokenContractCode(contract),
                phase: StatePhase::Before,
            },
        )?;

        let after_code_hash = after.contract_code_hashes.get(&contract).copied().ok_or(
            ContractStandardsError::StateValueMissing {
                requirement: StateRequirement::TokenContractCode(contract),
                phase: StatePhase::After,
            },
        )?;

        if before_code_hash != after_code_hash {
            return Err(ContractStandardsError::TokenContractCodeChanged {
                contract,
                before_code_hash,
                after_code_hash,
            });
        }
    }

    for &collection in &keys.collection_standards {
        let before_standards = collection_standards(before, collection, StatePhase::Before)?;
        let after_standards = collection_standards(after, collection, StatePhase::After)?;

        if before_standards != after_standards {
            return Err(ContractStandardsError::CollectionStandardsChanged {
                collection,
                before: before_standards,
                after: after_standards,
            });
        }
    }

    for candidate in candidates {
        match candidate.kind {
            StandardCandidateKind::Erc721Transfer { collection, .. }
            | StandardCandidateKind::Erc721Approval { collection, .. } => {
                let standards = collection_standards(before, collection, StatePhase::Before)?;

                if !standards.supports_erc721 {
                    return Err(ContractStandardsError::CollectionStandardNotSupported {
                        collection,
                        standard: "ERC-721",
                    });
                }
            }

            StandardCandidateKind::Erc1155Transfer { collection, .. } => {
                let standards = collection_standards(before, collection, StatePhase::Before)?;

                if !standards.supports_erc1155 {
                    return Err(ContractStandardsError::CollectionStandardNotSupported {
                        collection,
                        standard: "ERC-1155",
                    });
                }
            }

            StandardCandidateKind::OperatorApproval { collection, .. } => {
                let standards = collection_standards(before, collection, StatePhase::Before)?;

                if standards.supports_erc721 == standards.supports_erc1155 {
                    return Err(ContractStandardsError::OperatorApprovalStandardAmbiguous {
                        collection,
                        supports_erc721: standards.supports_erc721,
                        supports_erc1155: standards.supports_erc1155,
                    });
                }
            }

            StandardCandidateKind::Erc20Movement { .. }
            | StandardCandidateKind::Erc20Allowance { .. } => {}
        }
    }

    Ok(())
}

fn collection_standards(
    values: &StandardStateValues,
    collection: Address,
    phase: StatePhase,
) -> Result<CollectionStandards, ContractStandardsError> {
    values.collection_standards.get(&collection).copied().ok_or(
        ContractStandardsError::StateValueMissing {
            requirement: StateRequirement::CollectionStandards(collection),
            phase,
        },
    )
}

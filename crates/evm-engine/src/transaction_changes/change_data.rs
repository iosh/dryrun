use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;

use super::candidate::{ChangeCandidate, ChangeCandidateKind, Erc20AllowanceEvidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContractKind {
    Erc721,
    Erc1155,
    FungibleLike,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Erc20Metadata {
    pub(crate) name: Option<String>,
    pub(crate) symbol: Option<String>,
    pub(crate) decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Erc721CollectionMetadata {
    pub(crate) name: Option<String>,
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Erc721CollectionMetadataRequest {
    pub(crate) collection: Address,
    pub(crate) only_if_classified_as_erc721: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ChangeDataRequests {
    pub(crate) contract_kinds: Vec<Address>,
    pub(crate) erc20_metadata: Vec<Address>,
    pub(crate) erc721_collection_metadata: Vec<Erc721CollectionMetadataRequest>,
}

pub(crate) fn collect_change_data_requests(candidates: &[ChangeCandidate]) -> ChangeDataRequests {
    let explicit_erc721_collections = candidates
        .iter()
        .filter_map(|candidate| match candidate.kind {
            ChangeCandidateKind::Erc721Transfer { collection, .. }
            | ChangeCandidateKind::Erc721Approval { collection, .. } => Some(collection),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let mut requests = ChangeDataRequests::default();
    let mut seen_contract_kinds = HashSet::new();
    let mut seen_erc20_metadata = HashSet::new();
    let mut seen_erc721_metadata = HashSet::new();

    for candidate in candidates {
        match candidate.kind {
            ChangeCandidateKind::Erc20Transfer { token, .. }
            | ChangeCandidateKind::Erc20Allowance {
                token,
                evidence: Erc20AllowanceEvidence::ApprovalEvent { .. },
                ..
            } => {
                if seen_erc20_metadata.insert(token) {
                    requests.erc20_metadata.push(token);
                }
            }
            ChangeCandidateKind::Erc721Transfer { collection, .. }
            | ChangeCandidateKind::Erc721Approval { collection, .. } => {
                if seen_erc721_metadata.insert(collection) {
                    requests
                        .erc721_collection_metadata
                        .push(Erc721CollectionMetadataRequest {
                            collection,
                            only_if_classified_as_erc721: false,
                        });
                }
            }
            ChangeCandidateKind::OperatorApproval { collection, .. } => {
                if seen_contract_kinds.insert(collection) {
                    requests.contract_kinds.push(collection);
                }

                if seen_erc721_metadata.insert(collection) {
                    requests
                        .erc721_collection_metadata
                        .push(Erc721CollectionMetadataRequest {
                            collection,
                            only_if_classified_as_erc721: !explicit_erc721_collections
                                .contains(&collection),
                        });
                }
            }
            ChangeCandidateKind::NativeTransfer { .. }
            | ChangeCandidateKind::Erc1155Transfer { .. }
            | ChangeCandidateKind::Erc20Allowance {
                evidence: Erc20AllowanceEvidence::TransferFromCall { .. },
                ..
            } => {}
        }
    }

    requests
}

#[derive(Debug, Default)]
pub(crate) struct ChangeData {
    contract_kinds: HashMap<Address, ContractKind>,
    erc20_metadata: HashMap<Address, Erc20Metadata>,
    erc721_collection_metadata: HashMap<Address, Erc721CollectionMetadata>,
}

impl ChangeData {
    pub(crate) fn new(
        contract_kinds: HashMap<Address, ContractKind>,
        erc20_metadata: HashMap<Address, Erc20Metadata>,
        erc721_collection_metadata: HashMap<Address, Erc721CollectionMetadata>,
    ) -> Self {
        Self {
            contract_kinds,
            erc20_metadata,
            erc721_collection_metadata,
        }
    }

    pub(super) fn contract_kind(&self, contract: Address) -> ContractKind {
        self.contract_kinds
            .get(&contract)
            .copied()
            .unwrap_or(ContractKind::Unknown)
    }

    pub(super) fn erc20_metadata(&self, token: Address) -> Erc20Metadata {
        self.erc20_metadata.get(&token).cloned().unwrap_or_default()
    }

    pub(super) fn erc721_collection_metadata(
        &self,
        collection: Address,
    ) -> Erc721CollectionMetadata {
        self.erc721_collection_metadata
            .get(&collection)
            .cloned()
            .unwrap_or_default()
    }
}

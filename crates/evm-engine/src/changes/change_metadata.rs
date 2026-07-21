use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;

use crate::{Change, Erc20Metadata, Erc721CollectionMetadata, NativeMetadata};

use super::PositionedChange;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ChangeMetadataRequests {
    pub(crate) erc20_contracts: Vec<Address>,
    pub(crate) erc721_collections: Vec<Address>,
}

pub(crate) fn collect_change_metadata_requests(
    changes: &[PositionedChange],
) -> ChangeMetadataRequests {
    let mut requests = ChangeMetadataRequests::default();
    let mut seen_erc20 = HashSet::new();
    let mut seen_erc721 = HashSet::new();

    for positioned_change in changes {
        match &positioned_change.change {
            Change::Erc20Transfer {
                contract_address, ..
            }
            | Change::Erc20Mint {
                contract_address, ..
            }
            | Change::Erc20Burn {
                contract_address, ..
            }
            | Change::Erc20Allowance {
                contract_address, ..
            } => {
                if seen_erc20.insert(*contract_address) {
                    requests.erc20_contracts.push(*contract_address);
                }
            }
            Change::Erc721Transfer {
                contract_address, ..
            }
            | Change::Erc721Mint {
                contract_address, ..
            }
            | Change::Erc721Burn {
                contract_address, ..
            }
            | Change::Erc721TokenApproval {
                contract_address, ..
            }
            | Change::Erc721OperatorApproval {
                contract_address, ..
            } => {
                if seen_erc721.insert(*contract_address) {
                    requests.erc721_collections.push(*contract_address);
                }
            }
            Change::NativeTransfer { .. }
            | Change::Erc1155Transfer { .. }
            | Change::Erc1155Mint { .. }
            | Change::Erc1155Burn { .. }
            | Change::Erc1155OperatorApproval { .. } => {}
        }
    }

    requests
}

#[derive(Debug, Default)]
pub(crate) struct ChangeMetadata {
    native: NativeMetadata,
    erc20: HashMap<Address, Erc20Metadata>,
    erc721: HashMap<Address, Erc721CollectionMetadata>,
}

impl ChangeMetadata {
    pub(crate) fn new(
        native: NativeMetadata,
        erc20: HashMap<Address, Erc20Metadata>,
        erc721: HashMap<Address, Erc721CollectionMetadata>,
    ) -> Self {
        Self {
            native,
            erc20,
            erc721,
        }
    }

    pub(crate) fn enrich(&self, change: &mut Change) {
        match change {
            Change::NativeTransfer { metadata, .. } => {
                *metadata = self.native.clone();
            }
            Change::Erc20Transfer {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc20Mint {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc20Burn {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc20Allowance {
                contract_address,
                metadata,
                ..
            } => {
                *metadata = self
                    .erc20
                    .get(contract_address)
                    .cloned()
                    .unwrap_or_default();
            }
            Change::Erc721Transfer {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc721Mint {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc721Burn {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc721TokenApproval {
                contract_address,
                metadata,
                ..
            }
            | Change::Erc721OperatorApproval {
                contract_address,
                metadata,
                ..
            } => {
                *metadata = self
                    .erc721
                    .get(contract_address)
                    .cloned()
                    .unwrap_or_default();
            }
            Change::Erc1155Transfer { .. }
            | Change::Erc1155Mint { .. }
            | Change::Erc1155Burn { .. }
            | Change::Erc1155OperatorApproval { .. } => {}
        }
    }
}

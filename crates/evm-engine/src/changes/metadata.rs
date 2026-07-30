use contract_standards::StandardMetadata;

use crate::{Change, NativeMetadata};

#[derive(Debug, Default)]
pub(crate) struct ChangeMetadata {
    native: NativeMetadata,
    standard: StandardMetadata,
}

impl ChangeMetadata {
    pub(crate) fn new(native: NativeMetadata, standard: StandardMetadata) -> Self {
        Self { native, standard }
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
                    .standard
                    .erc20(contract_address)
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
                    .standard
                    .erc721(contract_address)
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

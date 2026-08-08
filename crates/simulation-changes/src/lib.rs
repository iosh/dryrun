use alloy_primitives::{Address, U256};
use contract_standards::legacy::{
    Change as StandardChange, Position, PositionedChange as PositionedStandardChange,
    StandardMetadata,
};

pub use contract_standards::{Erc20Metadata, Erc721CollectionMetadata};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeMetadata {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    NativeTransfer {
        from: Address,
        to: Address,
        raw_amount: U256,
        metadata: NativeMetadata,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Mint {
        contract_address: Address,
        to: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc20Burn {
        contract_address: Address,
        from: Address,
        raw_amount: U256,
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Mint {
        contract_address: Address,
        to: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc721Burn {
        contract_address: Address,
        from: Address,
        token_id: U256,
        metadata: Erc721CollectionMetadata,
    },
    Erc1155Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155Mint {
        contract_address: Address,
        to: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc1155Burn {
        contract_address: Address,
        from: Address,
        token_id: U256,
        raw_amount: U256,
    },
    Erc20Allowance {
        contract_address: Address,
        owner: Address,
        spender: Address,
        raw_amount_before: U256,
        raw_amount_after: U256,
        metadata: Erc20Metadata,
    },
    Erc721TokenApproval {
        contract_address: Address,
        token_id: U256,
        approved_address_before: Option<Address>,
        approved_address_after: Option<Address>,
        metadata: Erc721CollectionMetadata,
    },
    Erc721OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
        metadata: Erc721CollectionMetadata,
    },
    Erc1155OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedChange {
    position: Position,
    change: Change,
}

impl PositionedChange {
    pub const fn new(position: Position, change: Change) -> Self {
        Self { position, change }
    }
}

impl From<PositionedStandardChange> for PositionedChange {
    fn from(positioned: PositionedStandardChange) -> Self {
        Self::new(positioned.position, positioned.change.into())
    }
}

impl From<StandardChange> for Change {
    fn from(change: StandardChange) -> Self {
        match change {
            StandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
            } => Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata: Erc20Metadata::default(),
            },
            StandardChange::Erc20Mint {
                contract_address,
                to,
                raw_amount,
            } => Self::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata: Erc20Metadata::default(),
            },
            StandardChange::Erc20Burn {
                contract_address,
                from,
                raw_amount,
            } => Self::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata: Erc20Metadata::default(),
            },
            StandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
            } => Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata: Erc721CollectionMetadata::default(),
            },
            StandardChange::Erc721Mint {
                contract_address,
                to,
                token_id,
            } => Self::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata: Erc721CollectionMetadata::default(),
            },
            StandardChange::Erc721Burn {
                contract_address,
                from,
                token_id,
            } => Self::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata: Erc721CollectionMetadata::default(),
            },
            StandardChange::Erc1155Transfer {
                contract_address,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155Transfer {
                contract_address,
                from,
                to,
                token_id,
                raw_amount,
            },
            StandardChange::Erc1155Mint {
                contract_address,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155Mint {
                contract_address,
                to,
                token_id,
                raw_amount,
            },
            StandardChange::Erc1155Burn {
                contract_address,
                from,
                token_id,
                raw_amount,
            } => Self::Erc1155Burn {
                contract_address,
                from,
                token_id,
                raw_amount,
            },
            StandardChange::Erc20Allowance {
                contract_address,
                owner,
                spender,
                raw_amount_before,
                raw_amount_after,
            } => Self::Erc20Allowance {
                contract_address,
                owner,
                spender,
                raw_amount_before,
                raw_amount_after,
                metadata: Erc20Metadata::default(),
            },
            StandardChange::Erc721TokenApproval {
                contract_address,
                token_id,
                approved_address_before,
                approved_address_after,
            } => Self::Erc721TokenApproval {
                contract_address,
                token_id,
                approved_address_before,
                approved_address_after,
                metadata: Erc721CollectionMetadata::default(),
            },
            StandardChange::Erc721OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
            } => Self::Erc721OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
                metadata: Erc721CollectionMetadata::default(),
            },
            StandardChange::Erc1155OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
            } => Self::Erc1155OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct ChangeMetadata {
    native: NativeMetadata,
    standard: StandardMetadata,
}

impl ChangeMetadata {
    pub const fn new(native: NativeMetadata, standard: StandardMetadata) -> Self {
        Self { native, standard }
    }

    pub const fn native_metadata(&self) -> &NativeMetadata {
        &self.native
    }

    pub fn enrich_change(&self, change: &mut Change) {
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

pub fn sort_changes_by_position(changes: &mut [PositionedChange]) {
    changes.sort_by_key(|positioned| positioned.position);
}

pub fn into_enriched_changes(
    positioned_changes: Vec<PositionedChange>,
    metadata: &ChangeMetadata,
) -> Vec<Change> {
    positioned_changes
        .into_iter()
        .map(|mut positioned| {
            metadata.enrich_change(&mut positioned.change);
            positioned.change
        })
        .collect()
}

use alloy_primitives::{Address, U256};
use conflux_service::espace as service_espace;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "changeType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum Change {
    Transfer {
        #[serde(flatten)]
        asset: TransferAsset,
        from: Address,
        to: Address,
    },
    Mint {
        #[serde(flatten)]
        asset: TokenMovementAsset,
        to: Address,
    },
    Burn {
        #[serde(flatten)]
        asset: TokenMovementAsset,
        from: Address,
    },
    Allowance {
        #[serde(flatten)]
        asset: AllowanceAsset,
        owner: Address,
        spender: Address,
    },
    TokenApproval {
        #[serde(flatten)]
        asset: TokenApprovalAsset,
    },
    OperatorApproval {
        #[serde(flatten)]
        asset: OperatorApprovalAsset,
        owner: Address,
        operator: Address,
        approved_before: bool,
        approved_after: bool,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum TransferAsset {
    Native {
        raw_amount: U256,
        #[serde(flatten)]
        metadata: NativeMetadata,
    },
    Erc20 {
        contract_address: Address,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum TokenMovementAsset {
    Erc20 {
        contract_address: Address,
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        raw_amount: U256,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum AllowanceAsset {
    Erc20 {
        contract_address: Address,
        raw_amount_before: U256,
        raw_amount_after: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum TokenApprovalAsset {
    Erc721 {
        contract_address: Address,
        token_id: U256,
        approved_address_before: Option<Address>,
        approved_address_after: Option<Address>,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "assetType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum OperatorApprovalAsset {
    Erc721 {
        contract_address: Address,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc1155 {
        contract_address: Address,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Erc20Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Erc721CollectionMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
}

impl From<service_espace::NativeMetadata> for NativeMetadata {
    fn from(metadata: service_espace::NativeMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<service_espace::Erc20Metadata> for Erc20Metadata {
    fn from(metadata: service_espace::Erc20Metadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<service_espace::Erc721CollectionMetadata> for Erc721CollectionMetadata {
    fn from(metadata: service_espace::Erc721CollectionMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
        }
    }
}

impl From<service_espace::Change> for Change {
    fn from(change: service_espace::Change) -> Self {
        match change {
            service_espace::Change::NativeTransfer {
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Transfer {
                asset: TransferAsset::Native {
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            service_espace::Change::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Transfer {
                asset: TransferAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            service_espace::Change::Erc20Mint {
                contract_address,
                to,
                raw_amount,
                metadata,
            } => Self::Mint {
                asset: TokenMovementAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                to,
            },
            service_espace::Change::Erc20Burn {
                contract_address,
                from,
                raw_amount,
                metadata,
            } => Self::Burn {
                asset: TokenMovementAsset::Erc20 {
                    contract_address,
                    raw_amount,
                    metadata: metadata.into(),
                },
                from,
            },
            service_espace::Change::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Transfer {
                asset: TransferAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                from,
                to,
            },
            service_espace::Change::Erc721Mint {
                contract_address,
                to,
                token_id,
                metadata,
            } => Self::Mint {
                asset: TokenMovementAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                to,
            },
            service_espace::Change::Erc721Burn {
                contract_address,
                from,
                token_id,
                metadata,
            } => Self::Burn {
                asset: TokenMovementAsset::Erc721 {
                    contract_address,
                    token_id,
                    metadata: metadata.into(),
                },
                from,
            },
            service_espace::Change::Erc1155Transfer {
                contract_address,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Transfer {
                asset: TransferAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                from,
                to,
            },
            service_espace::Change::Erc1155Mint {
                contract_address,
                to,
                token_id,
                raw_amount,
            } => Self::Mint {
                asset: TokenMovementAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                to,
            },
            service_espace::Change::Erc1155Burn {
                contract_address,
                from,
                token_id,
                raw_amount,
            } => Self::Burn {
                asset: TokenMovementAsset::Erc1155 {
                    contract_address,
                    token_id,
                    raw_amount,
                },
                from,
            },
            service_espace::Change::Erc20Allowance {
                contract_address,
                owner,
                spender,
                raw_amount_before,
                raw_amount_after,
                metadata,
            } => Self::Allowance {
                asset: AllowanceAsset::Erc20 {
                    contract_address,
                    raw_amount_before,
                    raw_amount_after,
                    metadata: metadata.into(),
                },
                owner,
                spender,
            },
            service_espace::Change::Erc721TokenApproval {
                contract_address,
                token_id,
                approved_address_before,
                approved_address_after,
                metadata,
            } => Self::TokenApproval {
                asset: TokenApprovalAsset::Erc721 {
                    contract_address,
                    token_id,
                    approved_address_before,
                    approved_address_after,
                    metadata: metadata.into(),
                },
            },
            service_espace::Change::Erc721OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
                metadata,
            } => Self::OperatorApproval {
                asset: OperatorApprovalAsset::Erc721 {
                    contract_address,
                    metadata: metadata.into(),
                },
                owner,
                operator,
                approved_before,
                approved_after,
            },
            service_espace::Change::Erc1155OperatorApproval {
                contract_address,
                owner,
                operator,
                approved_before,
                approved_after,
            } => Self::OperatorApproval {
                asset: OperatorApprovalAsset::Erc1155 { contract_address },
                owner,
                operator,
                approved_before,
                approved_after,
            },
        }
    }
}

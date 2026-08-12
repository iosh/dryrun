use alloy_primitives::{Address, U256};
use conflux_simulation::espace::{
    Erc20Metadata as SimulationErc20Metadata,
    Erc721CollectionMetadata as SimulationErc721CollectionMetadata,
    Erc1155TransferItem as SimulationErc1155TransferItem, EspaceChange, EspaceNativeCurrency,
    StandardChange,
};
use serde::Serialize;

mod u256_hex {
    use alloy_primitives::U256;
    use serde::{Serialize, Serializer};

    pub(super) fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "changeType",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub(super) enum Change {
    NativeTransfer {
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    SelfDestructBurn {
        contract_address: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        currency: NativeCurrency,
    },
    WrappedNativeDeposit {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    WrappedNativeWithdrawal {
        contract_address: Address,
        account: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc20Approval {
        contract_address: Address,
        owner: Address,
        spender: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        approved_amount: U256,
        #[serde(flatten)]
        metadata: Erc20Metadata,
    },
    Erc721Transfer {
        contract_address: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    Erc721Approval {
        contract_address: Address,
        owner: Address,
        approved_address: Option<Address>,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(flatten)]
        metadata: Erc721CollectionMetadata,
    },
    OperatorApproval {
        contract_address: Address,
        owner: Address,
        operator: Address,
        approved: bool,
    },
    Erc1155TransferSingle {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        #[serde(serialize_with = "u256_hex::serialize")]
        token_id: U256,
        #[serde(serialize_with = "u256_hex::serialize")]
        raw_amount: U256,
    },
    Erc1155TransferBatch {
        contract_address: Address,
        operator: Address,
        from: Address,
        to: Address,
        items: Vec<Erc1155TransferItem>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Erc1155TransferItem {
    #[serde(serialize_with = "u256_hex::serialize")]
    token_id: U256,
    #[serde(serialize_with = "u256_hex::serialize")]
    raw_amount: U256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeCurrency {
    name: String,
    symbol: String,
    decimals: u8,
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

impl From<EspaceChange> for Change {
    fn from(change: EspaceChange) -> Self {
        match change {
            EspaceChange::NativeTransfer {
                from,
                to,
                raw_amount,
                currency,
            } => Self::NativeTransfer {
                from,
                to,
                raw_amount,
                currency: currency.into(),
            },
            EspaceChange::SelfDestructBurn {
                contract_address,
                raw_amount,
                currency,
            } => Self::SelfDestructBurn {
                contract_address,
                raw_amount,
                currency: currency.into(),
            },
            EspaceChange::WrappedNativeDeposit {
                contract_address,
                account,
                raw_amount,
                metadata,
            } => Self::WrappedNativeDeposit {
                contract_address,
                account,
                raw_amount,
                metadata: metadata.into(),
            },
            EspaceChange::WrappedNativeWithdrawal {
                contract_address,
                account,
                raw_amount,
                metadata,
            } => Self::WrappedNativeWithdrawal {
                contract_address,
                account,
                raw_amount,
                metadata: metadata.into(),
            },
            EspaceChange::Standard(change) => change.into(),
        }
    }
}

impl From<StandardChange<Address>> for Change {
    fn from(change: StandardChange<Address>) -> Self {
        match change {
            StandardChange::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata,
            } => Self::Erc20Transfer {
                contract_address,
                from,
                to,
                raw_amount,
                metadata: metadata.into(),
            },
            StandardChange::Erc20Approval {
                contract_address,
                owner,
                spender,
                approved_amount,
                metadata,
            } => Self::Erc20Approval {
                contract_address,
                owner,
                spender,
                approved_amount,
                metadata: metadata.into(),
            },
            StandardChange::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata,
            } => Self::Erc721Transfer {
                contract_address,
                from,
                to,
                token_id,
                metadata: metadata.into(),
            },
            StandardChange::Erc721Approval {
                contract_address,
                owner,
                approved_address,
                token_id,
                metadata,
            } => Self::Erc721Approval {
                contract_address,
                owner,
                approved_address,
                token_id,
                metadata: metadata.into(),
            },
            StandardChange::OperatorApproval {
                contract_address,
                owner,
                operator,
                approved,
            } => Self::OperatorApproval {
                contract_address,
                owner,
                operator,
                approved,
            },
            StandardChange::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            } => Self::Erc1155TransferSingle {
                contract_address,
                operator,
                from,
                to,
                token_id,
                raw_amount,
            },
            StandardChange::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items,
            } => Self::Erc1155TransferBatch {
                contract_address,
                operator,
                from,
                to,
                items: items.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<EspaceNativeCurrency> for NativeCurrency {
    fn from(currency: EspaceNativeCurrency) -> Self {
        Self {
            name: currency.name,
            symbol: currency.symbol,
            decimals: currency.decimals,
        }
    }
}

impl From<SimulationErc20Metadata> for Erc20Metadata {
    fn from(metadata: SimulationErc20Metadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
            decimals: metadata.decimals,
        }
    }
}

impl From<SimulationErc721CollectionMetadata> for Erc721CollectionMetadata {
    fn from(metadata: SimulationErc721CollectionMetadata) -> Self {
        Self {
            name: metadata.name,
            symbol: metadata.symbol,
        }
    }
}

impl From<SimulationErc1155TransferItem> for Erc1155TransferItem {
    fn from(item: SimulationErc1155TransferItem) -> Self {
        Self {
            token_id: item.token_id,
            raw_amount: item.raw_amount,
        }
    }
}

use cfx_rpc_eth_types::Bytes as RpcBytes;
use cfx_types::{Address, H256, U64, U256};
use conflux_service::espace as service_espace;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct SimulateEspaceTransactionResponse {
    execution: Execution,
    changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Execution {
    chain_id: U64,
    block: SimulatedBlock,
    status: SimulationStatus,
    gas_used: U256,
    gas_limit: U256,
    gas_charged: U256,
    fee: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    burnt_fee: Option<U256>,
    output: RpcBytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SimulatedBlock {
    number: U64,
    hash: H256,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
enum SimulationStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExecutionFailure {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
enum Change {
    Transfer {
        asset: Asset,
        from: Address,
        to: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Mint {
        asset: Asset,
        to: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Burn {
        asset: Asset,
        from: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    Approval {
        asset: Asset,
        owner: Address,
        spender: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        amount: Option<U256>,
    },
    ApprovalForAll {
        collection: Collection,
        owner: Address,
        operator: Address,
        approved: bool,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
enum Asset {
    Native {
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<NativeAssetDisplay>,
    },
    Erc20 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<Erc20AssetDisplay>,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc721CollectionDisplay>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<NftTokenDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc1155CollectionDisplay>,
        #[serde(skip_serializing_if = "Option::is_none")]
        token: Option<NftTokenDisplay>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct NativeAssetDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Erc20AssetDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Erc721CollectionDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Erc1155CollectionDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NftTokenDisplay {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
enum Collection {
    Erc721 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc721CollectionDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        #[serde(skip_serializing_if = "Option::is_none")]
        collection: Option<Erc1155CollectionDisplay>,
    },
}

impl From<service_espace::SimulateEspaceTransactionOutput> for SimulateEspaceTransactionResponse {
    fn from(output: service_espace::SimulateEspaceTransactionOutput) -> Self {
        Self {
            execution: output.execution.into(),
            changes: output.changes.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<service_espace::SimulationExecution> for Execution {
    fn from(execution: service_espace::SimulationExecution) -> Self {
        Self {
            chain_id: execution.chain_id.into(),
            block: execution.block.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            gas_charged: execution.gas_charged,
            fee: execution.fee,
            burnt_fee: execution.burnt_fee,
            output: RpcBytes::from(execution.output),
            error: execution.failure.map(Into::into),
        }
    }
}

impl From<service_espace::SimulatedBlock> for SimulatedBlock {
    fn from(block: service_espace::SimulatedBlock) -> Self {
        Self {
            number: block.number.into(),
            hash: block.hash,
        }
    }
}

impl From<service_espace::ExecutionStatus> for SimulationStatus {
    fn from(status: service_espace::ExecutionStatus) -> Self {
        match status {
            service_espace::ExecutionStatus::Success => Self::Success,
            service_espace::ExecutionStatus::Failed => Self::Failed,
        }
    }
}

impl From<service_espace::ExecutionFailure> for ExecutionFailure {
    fn from(failure: service_espace::ExecutionFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

impl From<service_espace::Change> for Change {
    fn from(change: service_espace::Change) -> Self {
        match change {
            service_espace::Change::Transfer(change) => Self::Transfer {
                asset: change.asset.into(),
                from: change.from,
                to: change.to,
                amount: change.amount,
            },
            service_espace::Change::Mint(change) => Self::Mint {
                asset: change.asset.into(),
                to: change.to,
                amount: change.amount,
            },
            service_espace::Change::Burn(change) => Self::Burn {
                asset: change.asset.into(),
                from: change.from,
                amount: change.amount,
            },
            service_espace::Change::Approval(change) => Self::Approval {
                asset: change.asset.into(),
                owner: change.owner,
                spender: change.spender,
                amount: change.amount,
            },
            service_espace::Change::ApprovalForAll(change) => Self::ApprovalForAll {
                collection: change.collection.into(),
                owner: change.owner,
                operator: change.operator,
                approved: change.approved,
            },
        }
    }
}

impl From<service_espace::Asset> for Asset {
    fn from(asset: service_espace::Asset) -> Self {
        match asset {
            service_espace::Asset::Native { display } => Self::Native {
                display: display.map(Into::into),
            },
            service_espace::Asset::Erc20 {
                contract_address,
                display,
            } => Self::Erc20 {
                contract_address,
                display: display.map(Into::into),
            },
            service_espace::Asset::Erc721 {
                contract_address,
                token_id,
                collection,
                token,
            } => Self::Erc721 {
                contract_address,
                token_id,
                collection: collection.map(Into::into),
                token: token.map(Into::into),
            },
            service_espace::Asset::Erc1155 {
                contract_address,
                token_id,
                collection,
                token,
            } => Self::Erc1155 {
                contract_address,
                token_id,
                collection: collection.map(Into::into),
                token: token.map(Into::into),
            },
        }
    }
}

impl From<service_espace::Collection> for Collection {
    fn from(collection: service_espace::Collection) -> Self {
        match collection {
            service_espace::Collection::Erc721 {
                contract_address,
                collection,
            } => Self::Erc721 {
                contract_address,
                collection: collection.map(Into::into),
            },
            service_espace::Collection::Erc1155 {
                contract_address,
                collection,
            } => Self::Erc1155 {
                contract_address,
                collection: collection.map(Into::into),
            },
        }
    }
}

impl From<service_espace::NativeAssetDisplay> for NativeAssetDisplay {
    fn from(display: service_espace::NativeAssetDisplay) -> Self {
        Self {
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<service_espace::Erc20AssetDisplay> for Erc20AssetDisplay {
    fn from(display: service_espace::Erc20AssetDisplay) -> Self {
        Self {
            name: display.name,
            symbol: display.symbol,
            decimals: display.decimals,
        }
    }
}

impl From<service_espace::Erc721CollectionDisplay> for Erc721CollectionDisplay {
    fn from(display: service_espace::Erc721CollectionDisplay) -> Self {
        Self {
            name: display.name,
            symbol: display.symbol,
        }
    }
}

impl From<service_espace::Erc1155CollectionDisplay> for Erc1155CollectionDisplay {
    fn from(display: service_espace::Erc1155CollectionDisplay) -> Self {
        Self { name: display.name }
    }
}

impl From<service_espace::NftTokenDisplay> for NftTokenDisplay {
    fn from(display: service_espace::NftTokenDisplay) -> Self {
        Self { name: display.name }
    }
}

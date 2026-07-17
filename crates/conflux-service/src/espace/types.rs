use cfx_types::{Address, U256};
use conflux_engine as engine;
pub use engine::espace::{
    AccessListItem, EspaceBlockRef, EspaceExecution, EspaceExecutionFailure,
    EspaceExecutionFailureCode, EspaceExecutionStatus, EspaceTransaction, EspaceTransactionVariant,
    SimulateEspaceTransactionInput, SimulatedBlock,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeAssetDisplay {
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erc20AssetDisplay {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erc721CollectionDisplay {
    pub name: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erc1155CollectionDisplay {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NftTokenDisplay {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asset {
    Native {
        display: Option<NativeAssetDisplay>,
    },
    Erc20 {
        contract_address: Address,
        display: Option<Erc20AssetDisplay>,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        collection: Option<Erc721CollectionDisplay>,
        token: Option<NftTokenDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        collection: Option<Erc1155CollectionDisplay>,
        token: Option<NftTokenDisplay>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Collection {
    Erc721 {
        contract_address: Address,
        collection: Option<Erc721CollectionDisplay>,
    },
    Erc1155 {
        contract_address: Address,
        collection: Option<Erc1155CollectionDisplay>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferChange {
    pub asset: Asset,
    pub from: Address,
    pub to: Address,
    pub amount: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintChange {
    pub asset: Asset,
    pub to: Address,
    pub amount: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurnChange {
    pub asset: Asset,
    pub from: Address,
    pub amount: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalChange {
    pub asset: Asset,
    pub owner: Address,
    pub spender: Address,
    pub amount: Option<U256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalForAllChange {
    pub collection: Collection,
    pub owner: Address,
    pub operator: Address,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Transfer(TransferChange),
    Mint(MintChange),
    Burn(BurnChange),
    Approval(ApprovalChange),
    ApprovalForAll(ApprovalForAllChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionOutput {
    pub execution: EspaceExecution,
}

impl From<engine::espace::EspaceExecution> for SimulateEspaceTransactionOutput {
    fn from(execution: engine::espace::EspaceExecution) -> Self {
        Self { execution }
    }
}

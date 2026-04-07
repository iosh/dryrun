use alloy_primitives::{Address, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asset {
    Native {
        symbol: Option<String>,
        decimals: Option<u8>,
    },
    Erc20 {
        contract_address: Address,
        symbol: Option<String>,
        decimals: Option<u8>,
        name: Option<String>,
    },
    Erc721 {
        contract_address: Address,
        token_id: U256,
        collection_name: Option<String>,
        name: Option<String>,
        symbol: Option<String>,
    },
    Erc1155 {
        contract_address: Address,
        token_id: U256,
        collection_name: Option<String>,
        name: Option<String>,
        symbol: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Collection {
    Erc721 {
        contract_address: Address,
        collection_name: Option<String>,
        name: Option<String>,
        symbol: Option<String>,
    },
    Erc1155 {
        contract_address: Address,
        collection_name: Option<String>,
        name: Option<String>,
        symbol: Option<String>,
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

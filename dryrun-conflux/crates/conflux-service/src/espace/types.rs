use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRef {
    Latest,
    Number(u64),
    Hash(H256),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EspaceTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
    Eip7702,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<H256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspaceTransaction {
    pub tx_type: EspaceTransactionType,
    pub requested_chain_id: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<U256>,
    pub gas_limit: U256,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub gas_price: Option<U256>,
    pub max_fee_per_gas: Option<U256>,
    pub max_priority_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

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
pub struct SimulationExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: ExecutionStatus,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub gas_charged: U256,
    pub fee: U256,
    pub burnt_fee: Option<U256>,
    pub output: Bytes,
    pub failure: Option<ExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionInput {
    pub block: BlockRef,
    pub transaction: EspaceTransaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulateEspaceTransactionOutput {
    pub execution: SimulationExecution,
    pub changes: Vec<Change>,
}

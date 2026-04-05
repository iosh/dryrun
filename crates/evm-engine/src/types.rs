use alloy_primitives::{Address, B256, Bytes, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockRef {
    Latest,
    Number(u64),
    Hash(B256),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmTransactionType {
    Legacy,
    AccessList,
    DynamicFee,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<B256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmTransaction {
    pub tx_type: EvmTransactionType,
    pub requested_chain_id: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: Option<u64>,
    pub gas_limit: u64,
    pub value: U256,
    pub data: Bytes,
    pub access_list: Vec<AccessListItem>,
    pub gas_price: Option<u128>,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionInput {
    pub block: BlockRef,
    pub transaction: EvmTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmExecutionStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedBlock {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecutionFailure {
    pub code: String,
    pub message: String,
    pub reason: Option<String>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmExecution {
    pub chain_id: u64,
    pub block: SimulatedBlock,
    pub status: EvmExecutionStatus,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub output: Bytes,
    pub failure: Option<EvmExecutionFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSimulation {
    execution: EvmExecution,
    changes: Vec<Change>,
}

impl EvmSimulation {
    pub fn new(execution: EvmExecution, changes: Vec<Change>) -> Self {
        Self { execution, changes }
    }

    pub fn execution(&self) -> &EvmExecution {
        &self.execution
    }

    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    pub fn into_parts(self) -> (EvmExecution, Vec<Change>) {
        (self.execution, self.changes)
    }
}

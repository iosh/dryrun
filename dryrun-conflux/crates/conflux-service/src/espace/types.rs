use cfx_bytes::Bytes;
use cfx_types::{Address, H256, U256};
use conflux_engine as engine;

use crate::ConfluxServiceError;

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

impl TryFrom<SimulateEspaceTransactionInput> for engine::SimulateEspaceTransactionInput {
    type Error = ConfluxServiceError;

    fn try_from(input: SimulateEspaceTransactionInput) -> Result<Self, Self::Error> {
        Ok(Self {
            block: map_block_ref(input.block)?,
            transaction: map_transaction(input.transaction),
        })
    }
}

impl From<engine::EspaceSimulation> for SimulateEspaceTransactionOutput {
    fn from(simulation: engine::EspaceSimulation) -> Self {
        Self {
            execution: simulation.into_execution().into(),
            changes: Vec::new(),
        }
    }
}

impl From<engine::EspaceExecution> for SimulationExecution {
    fn from(execution: engine::EspaceExecution) -> Self {
        Self {
            chain_id: execution.chain_id,
            block: execution.block.into(),
            status: execution.status.into(),
            gas_used: execution.gas_used,
            gas_limit: execution.gas_limit,
            gas_charged: execution.gas_charged,
            fee: execution.fee,
            burnt_fee: execution.burnt_fee,
            output: execution.output,
            failure: execution.failure.map(Into::into),
        }
    }
}

impl From<engine::SimulatedBlock> for SimulatedBlock {
    fn from(block: engine::SimulatedBlock) -> Self {
        Self {
            number: block.number,
            hash: block.hash,
        }
    }
}

impl From<engine::EspaceExecutionStatus> for ExecutionStatus {
    fn from(status: engine::EspaceExecutionStatus) -> Self {
        match status {
            engine::EspaceExecutionStatus::Success => Self::Success,
            engine::EspaceExecutionStatus::Failed => Self::Failed,
        }
    }
}

impl From<engine::EspaceExecutionFailure> for ExecutionFailure {
    fn from(failure: engine::EspaceExecutionFailure) -> Self {
        Self {
            code: failure.code,
            message: failure.message,
            reason: failure.reason,
        }
    }
}

fn map_block_ref(block: BlockRef) -> Result<engine::EspaceBlockRef, ConfluxServiceError> {
    match block {
        BlockRef::Latest => Ok(engine::EspaceBlockRef::Latest),
        BlockRef::Number(number) => Ok(engine::EspaceBlockRef::Number(number)),
        BlockRef::Hash(_) => Err(ConfluxServiceError::NotSupported {
            message: "eSpace block hash selectors are not supported yet".to_string(),
        }),
    }
}

fn map_transaction(transaction: EspaceTransaction) -> engine::EspaceTransaction {
    engine::EspaceTransaction {
        tx_type: transaction.tx_type.into(),
        requested_chain_id: transaction.requested_chain_id,
        from: transaction.from,
        to: transaction.to,
        nonce: transaction.nonce,
        gas_limit: transaction.gas_limit,
        value: transaction.value,
        data: transaction.data,
        access_list: transaction
            .access_list
            .into_iter()
            .map(|item| engine::AccessListItem {
                address: item.address,
                storage_keys: item.storage_keys,
            })
            .collect(),
        gas_price: transaction.gas_price,
        max_fee_per_gas: transaction.max_fee_per_gas,
        max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,
    }
}

impl From<EspaceTransactionType> for engine::EspaceTransactionType {
    fn from(tx_type: EspaceTransactionType) -> Self {
        match tx_type {
            EspaceTransactionType::Legacy => Self::Legacy,
            EspaceTransactionType::AccessList => Self::AccessList,
            EspaceTransactionType::DynamicFee => Self::DynamicFee,
            EspaceTransactionType::Eip7702 => Self::Eip7702,
        }
    }
}

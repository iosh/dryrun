use cfx_types::{Address, H256, SpaceMap, U256};
use primitives::BlockNumber;
use thiserror::Error;

use crate::state::{EspaceRpcBlock, NativeRpcBlock};

// Used by Native Context / PoS internal contracts. Upstream derives these
// values from a pivot block's PoS reference; ordinary eSpace execution does not
// depend on them, so the RPC-backed path keeps them optional until that
// resolution is implemented.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionConsensusContext {
    pub pos_view: Option<u64>,
    pub finalized_epoch: Option<u64>,
}

// Native and eSpace base fees come from different public RPC block views.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionBaseFees {
    pub native_base_fee_per_gas: Option<U256>,
    pub espace_base_fee_per_gas: Option<U256>,
}

impl ExecutionBaseFees {
    pub fn into_space_map(self) -> SpaceMap<U256> {
        SpaceMap::new(
            self.native_base_fee_per_gas.unwrap_or(U256::zero()),
            self.espace_base_fee_per_gas.unwrap_or(U256::zero()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct NativePivotBlockContext {
    pub(crate) block_number: BlockNumber,
    pub(crate) epoch_height: u64,
    pub(crate) author: Address,
    pub(crate) timestamp: u64,
    pub(crate) hash: H256,
    pub(crate) base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Copy)]
pub struct EspaceBlockContext {
    pub(crate) base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone)]
pub struct ExecutionBlockContext {
    pub(crate) pivot_block_number: BlockNumber,
    pub(crate) pivot_epoch_height: u64,
    pub(crate) author: Address,
    pub(crate) timestamp: u64,
    pub(crate) epoch_hash: H256,
    pub(crate) consensus: ExecutionConsensusContext,
    pub(crate) base_fees: ExecutionBaseFees,
}

#[derive(Debug, Error)]
pub enum ExecutionBlockContextError {
    #[error("native pivot block is missing blockNumber")]
    MissingBlockNumber,
    #[error("native pivot block {field} exceeds u64: {value:?}")]
    U64Overflow { field: &'static str, value: U256 },
}

pub fn build_native_pivot_block_context(
    block: NativeRpcBlock,
) -> Result<NativePivotBlockContext, ExecutionBlockContextError> {
    Ok(NativePivotBlockContext {
        block_number: required_block_number(block.block_number)?,
        epoch_height: u256_to_u64(block.height, "height")?,
        author: block.miner.hex_address,
        timestamp: u256_to_u64(block.timestamp, "timestamp")?,
        hash: block.hash,
        base_fee_per_gas: block.base_fee_per_gas,
    })
}

pub fn build_espace_block_context(block: EspaceRpcBlock) -> EspaceBlockContext {
    EspaceBlockContext {
        base_fee_per_gas: block.base_fee_per_gas,
    }
}

pub fn build_execution_block_context(
    pivot: NativePivotBlockContext,
    espace: EspaceBlockContext,
    consensus: ExecutionConsensusContext,
) -> ExecutionBlockContext {
    ExecutionBlockContext {
        pivot_block_number: pivot.block_number,
        pivot_epoch_height: pivot.epoch_height,
        author: pivot.author,
        timestamp: pivot.timestamp,
        epoch_hash: pivot.hash,
        consensus,
        base_fees: ExecutionBaseFees {
            native_base_fee_per_gas: pivot.base_fee_per_gas,
            espace_base_fee_per_gas: espace.base_fee_per_gas,
        },
    }
}

fn required_block_number(value: Option<U256>) -> Result<BlockNumber, ExecutionBlockContextError> {
    value
        .ok_or(ExecutionBlockContextError::MissingBlockNumber)
        .and_then(|value| u256_to_u64(value, "blockNumber"))
}

fn u256_to_u64(value: U256, field: &'static str) -> Result<u64, ExecutionBlockContextError> {
    if value > U256::from(u64::MAX) {
        return Err(ExecutionBlockContextError::U64Overflow { field, value });
    }

    Ok(value.as_u64())
}

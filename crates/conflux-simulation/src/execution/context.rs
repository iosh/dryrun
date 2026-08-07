use cfx_types::{Address, H256, SpaceMap, U256};
use primitives::BlockNumber;
use thiserror::Error;

use crate::state::{CoreSpaceRpcBlock, EspaceRpcBlock};

// Core preparation resolves these values from the pivot block's PoS reference.
// They remain optional because pre-PoS pivots have no such reference and ordinary
// eSpace execution does not depend on this consensus context.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ExecutionConsensusContext {
    pub pos_view: Option<u64>,
    pub finalized_epoch: Option<u64>,
}

// Core Space and eSpace base fees come from different public RPC block views.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExecutionBaseFees {
    pub core_space_base_fee_per_gas: Option<U256>,
    pub espace_base_fee_per_gas: Option<U256>,
}

impl ExecutionBaseFees {
    pub(crate) fn into_space_map(self) -> SpaceMap<U256> {
        SpaceMap::new(
            self.core_space_base_fee_per_gas.unwrap_or(U256::zero()),
            self.espace_base_fee_per_gas.unwrap_or(U256::zero()),
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CoreSpacePivotBlockContext {
    pub(crate) block_number: BlockNumber,
    pub(crate) epoch_height: u64,
    pub(crate) author: Address,
    pub(crate) timestamp: u64,
    pub(crate) hash: H256,
    pub(crate) base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EspaceBlockContext {
    pub(crate) base_fee_per_gas: Option<U256>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionBlockContext {
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
    #[error("Core Space pivot block is missing blockNumber")]
    MissingBlockNumber,
    #[error("Core Space pivot block {field} exceeds u64: {value:?}")]
    U64Overflow { field: &'static str, value: U256 },
    #[error("next execution block number overflows u64 after {pivot_block_number}")]
    NextBlockNumberOverflow { pivot_block_number: BlockNumber },
    #[error("next execution epoch height overflows u64 after {pivot_epoch_height}")]
    NextEpochHeightOverflow { pivot_epoch_height: u64 },
}

pub(crate) fn build_core_space_pivot_block_context(
    block: &CoreSpaceRpcBlock,
) -> Result<CoreSpacePivotBlockContext, ExecutionBlockContextError> {
    Ok(CoreSpacePivotBlockContext {
        block_number: required_block_number(block.block_number)?,
        epoch_height: u256_to_u64(block.height, "height")?,
        author: block.miner.hex_address,
        timestamp: u256_to_u64(block.timestamp, "timestamp")?,
        hash: block.hash,
        base_fee_per_gas: block.base_fee_per_gas,
    })
}

pub(crate) fn build_espace_block_context(block: &EspaceRpcBlock) -> EspaceBlockContext {
    EspaceBlockContext {
        base_fee_per_gas: block.base_fee_per_gas,
    }
}

pub(crate) fn build_execution_block_context(
    pivot: &CoreSpacePivotBlockContext,
    espace: &EspaceBlockContext,
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
            core_space_base_fee_per_gas: pivot.base_fee_per_gas,
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

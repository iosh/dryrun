use cfx_executor::spec::CommonParams;
use cfx_types::{Address, H256, Space, SpaceMap, U256};
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
    fn resolve_for_execution(
        self,
        params: &CommonParams,
        execution_epoch_height: u64,
    ) -> Result<SpaceMap<U256>, ExecutionBlockContextError> {
        let activation = params.transition_heights.cip1559;
        if execution_epoch_height < activation {
            return Ok(SpaceMap::new(U256::zero(), U256::zero()));
        }
        if execution_epoch_height == activation {
            return Ok(params.init_base_price());
        }

        let core_space = self.core_space_base_fee_per_gas.ok_or(
            ExecutionBlockContextError::MissingCoreSpaceBaseFee {
                execution_epoch_height,
            },
        )?;
        let espace = self.espace_base_fee_per_gas.ok_or(
            ExecutionBlockContextError::MissingEspaceBaseFee {
                execution_epoch_height,
            },
        )?;
        Ok(SpaceMap::new(core_space, espace))
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
pub(crate) struct EspaceExecutionBlockContext {
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

impl ExecutionBlockContext {
    pub(crate) fn resolve_base_fees(
        &mut self,
        params: &CommonParams,
        execution_epoch_height: u64,
    ) -> Result<(), ExecutionBlockContextError> {
        let base_fees = self
            .base_fees
            .resolve_for_execution(params, execution_epoch_height)?;
        self.base_fees = ExecutionBaseFees {
            core_space_base_fee_per_gas: Some(base_fees[Space::Native]),
            espace_base_fee_per_gas: Some(base_fees[Space::Ethereum]),
        };
        Ok(())
    }

    pub(crate) fn base_fees_for_execution(
        &self,
        params: &CommonParams,
        execution_epoch_height: u64,
    ) -> Result<SpaceMap<U256>, ExecutionBlockContextError> {
        self.base_fees
            .resolve_for_execution(params, execution_epoch_height)
    }
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
    #[error(
        "Core Space base fee is missing for execution epoch {execution_epoch_height} after CIP-1559 activation"
    )]
    MissingCoreSpaceBaseFee { execution_epoch_height: u64 },
    #[error(
        "eSpace base fee is missing for execution epoch {execution_epoch_height} after CIP-1559 activation"
    )]
    MissingEspaceBaseFee { execution_epoch_height: u64 },
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

pub(crate) fn build_espace_execution_block_context(
    block: &EspaceRpcBlock,
) -> EspaceExecutionBlockContext {
    EspaceExecutionBlockContext {
        base_fee_per_gas: block.base_fee_per_gas,
    }
}

pub(crate) fn build_execution_block_context(
    pivot: &CoreSpacePivotBlockContext,
    espace: &EspaceExecutionBlockContext,
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

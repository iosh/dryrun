use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_types::{H256, U256};
use conflux_provider::BlockHashOrEpochNumber;

use crate::{
    ConfluxSimulationError,
    core_space::CoreSpaceEpochRef,
    execution::{
        CoreSpacePivotBlockContext, ExecutionBlockContext, ExecutionConsensusContext,
        build_core_space_pivot_block_context, build_espace_execution_block_context,
        build_execution_block_context,
    },
    primitive::b256_to_cfx,
    state::{
        ConfluxSimulationProvider, ConfluxStateAnchor, CoreSpaceRpcBlock, CoreSpaceRpcPoSBlock,
        EspaceRpcBlock,
    },
};

pub(crate) struct CoreSpaceSimulationContext {
    pub(crate) block_context: ExecutionBlockContext,
    pub(crate) state_anchor: ConfluxStateAnchor,
}

impl CoreSpaceSimulationContext {
    pub fn epoch_height(&self) -> u64 {
        self.block_context.pivot_epoch_height
    }

    pub fn base_fee_per_gas(&self) -> Option<U256> {
        self.block_context.base_fees.core_space_base_fee_per_gas
    }

    pub fn state_epoch(&self) -> CfxEpochNumber {
        self.state_anchor.core_space_epoch()
    }

    pub fn state_pivot(&self) -> BlockHashOrEpochNumber {
        self.state_anchor.core_space_pivot()
    }
}

pub(crate) async fn load_core_space_context(
    provider: &ConfluxSimulationProvider,
    epoch: &CoreSpaceEpochRef,
) -> Result<CoreSpaceSimulationContext, ConfluxSimulationError> {
    let core_space_pivot_block = provider
        .cfx_get_block_by_epoch_number(core_space_epoch_selector(epoch))
        .await?
        .ok_or_else(|| ConfluxSimulationError::BlockNotFound {
            block: "Core Space pivot block".to_string(),
        })?;

    let core_space_pivot = build_core_space_pivot_block_context(&core_space_pivot_block)?;
    let state_anchor = state_anchor_from_core_space_pivot(&core_space_pivot);
    let espace_block = load_espace_block(provider, state_anchor).await?;
    validate_same_state_anchor(state_anchor, state_anchor_from_espace_block(&espace_block))?;
    let espace = build_espace_execution_block_context(&espace_block);
    let consensus = load_core_space_consensus_context(provider, &core_space_pivot_block).await?;
    let block_context = build_execution_block_context(&core_space_pivot, &espace, consensus);

    Ok(CoreSpaceSimulationContext {
        block_context,
        state_anchor,
    })
}

async fn load_core_space_consensus_context(
    provider: &ConfluxSimulationProvider,
    pivot_block: &CoreSpaceRpcBlock,
) -> Result<ExecutionConsensusContext, ConfluxSimulationError> {
    let Some(pos_reference) = pivot_block.pos_reference else {
        return Ok(ExecutionConsensusContext::default());
    };

    let pos_block = provider
        .pos_get_block_by_hash(pos_reference)
        .await?
        .ok_or_else(|| ConfluxSimulationError::BlockNotFound {
            block: format!("PoS block referenced by Core Space pivot {pos_reference:?}"),
        })?;

    consensus_context_from_pos_block(pos_reference, pos_block)
}

fn consensus_context_from_pos_block(
    pos_reference: H256,
    pos_block: CoreSpaceRpcPoSBlock,
) -> Result<ExecutionConsensusContext, ConfluxSimulationError> {
    if pos_block.hash != pos_reference {
        return Err(ConfluxSimulationError::InvalidBlockContext {
            message: format!(
                "PoS block hash does not match Core Space pivot reference: expected {pos_reference:?}, got {:?}",
                pos_block.hash
            ),
        });
    }

    let finalized_epoch = pos_block
        .pivot_decision
        .ok_or_else(|| ConfluxSimulationError::InvalidBlockContext {
            message: format!("referenced PoS block {pos_reference:?} is missing pivotDecision"),
        })?
        .height
        .as_u64();

    Ok(ExecutionConsensusContext {
        pos_view: Some(pos_block.height.as_u64()),
        finalized_epoch: Some(finalized_epoch),
    })
}

async fn load_espace_block(
    provider: &ConfluxSimulationProvider,
    anchor: ConfluxStateAnchor,
) -> Result<EspaceRpcBlock, ConfluxSimulationError> {
    provider
        .eth_get_block(anchor.espace_block())
        .await?
        .ok_or_else(|| ConfluxSimulationError::BlockNotFound {
            block: "eSpace block".to_string(),
        })
}

fn core_space_epoch_selector(epoch: &CoreSpaceEpochRef) -> CfxEpochNumber {
    match epoch {
        CoreSpaceEpochRef::LatestState => CfxEpochNumber::LatestState,
        CoreSpaceEpochRef::Number(number) => CfxEpochNumber::Num((*number).into()),
    }
}

fn state_anchor_from_espace_block(block: &EspaceRpcBlock) -> ConfluxStateAnchor {
    ConfluxStateAnchor::new(block.number, b256_to_cfx(block.hash))
}

fn state_anchor_from_core_space_pivot(pivot: &CoreSpacePivotBlockContext) -> ConfluxStateAnchor {
    ConfluxStateAnchor::new(pivot.epoch_height, pivot.hash)
}

fn validate_same_state_anchor(
    expected: ConfluxStateAnchor,
    actual: ConfluxStateAnchor,
) -> Result<(), ConfluxSimulationError> {
    if actual != expected {
        return Err(ConfluxSimulationError::StateAnchorInconsistent);
    }

    Ok(())
}

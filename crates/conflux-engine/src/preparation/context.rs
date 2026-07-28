use cfx_rpc_cfx_types::EpochNumber as CfxEpochNumber;
use cfx_rpc_eth_types::BlockId as EthBlockId;
use cfx_types::U256;

use crate::{
    ConfluxEngineError,
    core_space::CoreSpaceEpochRef,
    espace::{EspaceBlockRef, SimulatedBlock},
    execution::{
        CoreSpacePivotBlockContext, ExecutionBlockContext, ExecutionConsensusContext,
        build_core_space_pivot_block_context, build_espace_block_context,
        build_execution_block_context,
    },
    state::{ConfluxStateAnchor, CoreSpaceRpcBlock, EspaceRpcBlock, HttpConfluxProvider},
};

pub struct EspaceSimulationContext {
    pub(crate) block_context: ExecutionBlockContext,
    pub(crate) state_anchor: ConfluxStateAnchor,
    pub(crate) simulated_block: SimulatedBlock,
}

impl EspaceSimulationContext {
    pub fn base_fee_per_gas(&self) -> Option<U256> {
        self.block_context.base_fees.espace_base_fee_per_gas
    }

    pub fn state_block(&self) -> EthBlockId {
        self.state_anchor.espace_block()
    }
}

pub struct CoreSpaceSimulationContext {
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
}

pub(crate) async fn load_espace_context(
    provider: &HttpConfluxProvider,
    block: &EspaceBlockRef,
) -> Result<EspaceSimulationContext, ConfluxEngineError> {
    let espace_block = provider
        .eth_get_block_by_number(espace_block_selector(block))
        .await?
        .ok_or_else(|| ConfluxEngineError::BlockNotFound {
            block: "eSpace block".to_string(),
        })?;
    let state_anchor = state_anchor_from_espace_block(&espace_block)?;
    let core_space_pivot_block = load_core_space_pivot_block(provider, state_anchor).await?;

    let simulated_block = SimulatedBlock {
        number: state_anchor.epoch_number(),
        hash: espace_block.hash,
    };

    let core_space_pivot = build_core_space_pivot_block_context(&core_space_pivot_block)?;
    validate_same_state_anchor(
        state_anchor,
        state_anchor_from_core_space_pivot(&core_space_pivot),
    )?;
    let espace = build_espace_block_context(&espace_block);

    let block_context = build_execution_block_context(
        &core_space_pivot,
        &espace,
        ExecutionConsensusContext::default(),
    );

    Ok(EspaceSimulationContext {
        block_context,
        state_anchor,
        simulated_block,
    })
}

pub(crate) async fn load_core_space_context(
    provider: &HttpConfluxProvider,
    epoch: &CoreSpaceEpochRef,
) -> Result<CoreSpaceSimulationContext, ConfluxEngineError> {
    let core_space_pivot_block = provider
        .cfx_get_block_by_epoch_number(core_space_epoch_selector(epoch))
        .await?
        .ok_or_else(|| ConfluxEngineError::BlockNotFound {
            block: "Core Space pivot block".to_string(),
        })?;

    let core_space_pivot = build_core_space_pivot_block_context(&core_space_pivot_block)?;
    let state_anchor = state_anchor_from_core_space_pivot(&core_space_pivot);
    let espace_block = load_espace_block(provider, state_anchor).await?;
    validate_same_state_anchor(state_anchor, state_anchor_from_espace_block(&espace_block)?)?;
    let espace = build_espace_block_context(&espace_block);
    let block_context = build_execution_block_context(
        &core_space_pivot,
        &espace,
        ExecutionConsensusContext::default(),
    );

    Ok(CoreSpaceSimulationContext {
        block_context,
        state_anchor,
    })
}

async fn load_core_space_pivot_block(
    provider: &HttpConfluxProvider,
    anchor: ConfluxStateAnchor,
) -> Result<CoreSpaceRpcBlock, ConfluxEngineError> {
    provider
        .cfx_get_block_by_epoch_number(anchor.core_space_epoch())
        .await?
        .ok_or_else(|| ConfluxEngineError::BlockNotFound {
            block: "Core Space pivot block".to_string(),
        })
}

async fn load_espace_block(
    provider: &HttpConfluxProvider,
    anchor: ConfluxStateAnchor,
) -> Result<EspaceRpcBlock, ConfluxEngineError> {
    provider
        .eth_get_block_by_number(anchor.espace_block())
        .await?
        .ok_or_else(|| ConfluxEngineError::BlockNotFound {
            block: "eSpace block".to_string(),
        })
}

fn espace_block_selector(block: &EspaceBlockRef) -> EthBlockId {
    match block {
        EspaceBlockRef::Latest => EthBlockId::Latest,
        EspaceBlockRef::Number(number) => EthBlockId::Num(*number),
    }
}

fn core_space_epoch_selector(epoch: &CoreSpaceEpochRef) -> CfxEpochNumber {
    match epoch {
        CoreSpaceEpochRef::LatestState => CfxEpochNumber::LatestState,
        CoreSpaceEpochRef::Number(number) => CfxEpochNumber::Num((*number).into()),
    }
}

fn state_anchor_from_espace_block(
    block: &EspaceRpcBlock,
) -> Result<ConfluxStateAnchor, ConfluxEngineError> {
    Ok(ConfluxStateAnchor::new(
        espace_block_number(block)?,
        block.hash,
    ))
}

fn state_anchor_from_core_space_pivot(pivot: &CoreSpacePivotBlockContext) -> ConfluxStateAnchor {
    ConfluxStateAnchor::new(pivot.epoch_height, pivot.hash)
}

fn validate_same_state_anchor(
    expected: ConfluxStateAnchor,
    actual: ConfluxStateAnchor,
) -> Result<(), ConfluxEngineError> {
    if actual != expected {
        return Err(ConfluxEngineError::StateAnchorInconsistent);
    }

    Ok(())
}

fn espace_block_number(block: &EspaceRpcBlock) -> Result<u64, ConfluxEngineError> {
    if block.number > U256::from(u64::MAX) {
        return Err(ConfluxEngineError::InvalidBlockContext {
            message: format!("eSpace block number exceeds u64: {:?}", block.number),
        });
    }

    Ok(block.number.as_u64())
}

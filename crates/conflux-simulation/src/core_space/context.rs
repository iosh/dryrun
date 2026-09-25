use std::fmt;

use alloy_primitives::B256;
use cfx_executor::spec::CommonParams;
use cfx_types::{Space, U256};
use conflux_provider::{BlockHashOrEpochNumber, EpochNumber};
use thiserror::Error;

use crate::{
    ConfluxRpcError,
    context::{ConfluxBlockContextError, ConsensusContext, ExecutionBlockContext, PivotBlock},
    primitive::b256_from_cfx,
    state::{ConfluxSimulationProvider, ConfluxStateAnchor, CoreSpaceRpcBlock, EspaceRpcBlock},
};

/// Selects the Core Space pivot used by one simulation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreSpaceBlockSelector {
    /// Resolves the endpoint's latest pivot with available state once.
    LatestState,
    /// Resolves the pivot at the given epoch number.
    Number(u64),
    /// Resolves and verifies the pivot with the given hash.
    PivotHash(B256),
}

impl fmt::Display for CoreSpaceBlockSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LatestState => formatter.write_str("latest_state"),
            Self::Number(number) => write!(formatter, "epoch {number}"),
            Self::PivotHash(hash) => write!(formatter, "pivot hash {hash:#x}"),
        }
    }
}

/// The immutable Core Space pivot identity used by a simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CoreSpaceBlockContext {
    /// The resolved Core Space epoch number.
    pub epoch_number: u64,
    /// The resolved Core Space pivot hash.
    pub pivot_hash: B256,
}

impl CoreSpaceBlockContext {
    pub(crate) fn state_anchor(&self) -> ConfluxStateAnchor {
        ConfluxStateAnchor::new(
            self.epoch_number,
            crate::primitive::b256_to_cfx(self.pivot_hash),
        )
    }
}

/// An error resolving the fixed context for a Core Space simulation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoreSpaceContextError {
    /// A provider request required to resolve the context failed.
    #[error(transparent)]
    Rpc(#[from] ConfluxRpcError),

    /// The fixed pivot could not be verified before execution or result delivery.
    #[error(transparent)]
    StateAnchor(#[from] crate::ConfluxStateAnchorError),

    /// The selected Core Space block does not exist.
    #[error("Core Space pivot selected by {selector} was not found")]
    PivotBlockNotFound {
        /// The selector that did not resolve.
        selector: CoreSpaceBlockSelector,
    },

    /// The selected blocks cannot provide the required execution context.
    #[error(transparent)]
    BlockContext(#[from] ConfluxBlockContextError),

    /// A hash selector resolved to a non-pivot DAG block.
    #[error(
        "Core Space block {selected_hash:#x} is not the pivot at epoch {epoch_number}; the pivot is {actual_pivot_hash:#x}"
    )]
    SelectedBlockIsNotPivot {
        /// The selected block hash.
        selected_hash: B256,
        /// The epoch assigned to the selected block.
        epoch_number: u64,
        /// The actual pivot hash at that epoch.
        actual_pivot_hash: B256,
    },

    /// The dependent eSpace block does not exist.
    #[error(
        "eSpace block for Core Space pivot {pivot_hash:#x} at epoch {epoch_number} was not found"
    )]
    EspaceBlockNotFound {
        /// The fixed Core Space epoch number.
        epoch_number: u64,
        /// The fixed Core Space pivot hash.
        pivot_hash: B256,
    },

    /// The PoS facts required to reconstruct the execution context are unavailable.
    #[error(
        "PoS context {pos_reference:#x} referenced by Core Space pivot {pivot_hash:#x} is unavailable"
    )]
    ConsensusContextUnavailable {
        /// The fixed Core Space pivot hash.
        pivot_hash: B256,
        /// The referenced PoS block hash.
        pos_reference: B256,
    },
}

pub(crate) struct CoreSpaceContext {
    pub(crate) execution_block_context: ExecutionBlockContext,
    pub(crate) state_anchor: ConfluxStateAnchor,
    pub(crate) public_context: CoreSpaceBlockContext,
}

impl CoreSpaceContext {
    pub(crate) fn base_fee_per_gas(&self) -> U256 {
        self.execution_block_context.base_gas_price[Space::Native]
    }

    pub(crate) fn state_pivot(&self) -> BlockHashOrEpochNumber {
        self.state_anchor.core_space_pivot()
    }
}

pub(crate) async fn prepare_core_space_context(
    provider: &ConfluxSimulationProvider,
    selector: CoreSpaceBlockSelector,
    params: &CommonParams,
) -> Result<CoreSpaceContext, CoreSpaceContextError> {
    let selected_block = load_selected_block(provider, selector).await?;
    let selected_pivot = PivotBlock::try_from(&selected_block)?;
    let (pivot_block, pivot) =
        verify_hash_selected_pivot(provider, selector, selected_block, selected_pivot).await?;

    let state_anchor = ConfluxStateAnchor::new(pivot.epoch_number, pivot.hash);
    let public_context = CoreSpaceBlockContext {
        epoch_number: pivot.epoch_number,
        pivot_hash: b256_from_cfx(pivot.hash),
    };
    let espace_block = load_espace_block(provider, public_context, state_anchor).await?;
    let consensus = load_consensus_context(provider, public_context, &pivot_block).await?;
    let execution_block_context =
        ExecutionBlockContext::from_pivot(&pivot, &espace_block, consensus, params)?;

    Ok(CoreSpaceContext {
        execution_block_context,
        state_anchor,
        public_context,
    })
}

async fn load_selected_block(
    provider: &ConfluxSimulationProvider,
    selector: CoreSpaceBlockSelector,
) -> Result<CoreSpaceRpcBlock, CoreSpaceContextError> {
    let block = match selector {
        CoreSpaceBlockSelector::LatestState => {
            provider
                .cfx_get_block_by_epoch_number(EpochNumber::LatestState)
                .await
        }
        CoreSpaceBlockSelector::Number(number) => {
            provider
                .cfx_get_block_by_epoch_number(EpochNumber::Number(number))
                .await
        }
        CoreSpaceBlockSelector::PivotHash(hash) => provider.cfx_get_block_by_hash(hash).await,
    }?;

    block.ok_or(CoreSpaceContextError::PivotBlockNotFound { selector })
}

async fn verify_hash_selected_pivot(
    provider: &ConfluxSimulationProvider,
    selector: CoreSpaceBlockSelector,
    selected_block: CoreSpaceRpcBlock,
    selected_pivot: PivotBlock,
) -> Result<(CoreSpaceRpcBlock, PivotBlock), CoreSpaceContextError> {
    let CoreSpaceBlockSelector::PivotHash(selected_hash) = selector else {
        return Ok((selected_block, selected_pivot));
    };
    let epoch_number = selected_pivot.epoch_number;
    let pivot_block = provider
        .cfx_get_block_by_epoch_number(EpochNumber::Number(epoch_number))
        .await?
        .ok_or(CoreSpaceContextError::PivotBlockNotFound {
            selector: CoreSpaceBlockSelector::Number(epoch_number),
        })?;
    let pivot = PivotBlock::try_from(&pivot_block)?;
    let actual_pivot_hash = b256_from_cfx(pivot.hash);
    if actual_pivot_hash != selected_hash {
        return Err(CoreSpaceContextError::SelectedBlockIsNotPivot {
            selected_hash,
            epoch_number,
            actual_pivot_hash,
        });
    }
    Ok((pivot_block, pivot))
}

async fn load_espace_block(
    provider: &ConfluxSimulationProvider,
    context: CoreSpaceBlockContext,
    anchor: ConfluxStateAnchor,
) -> Result<EspaceRpcBlock, CoreSpaceContextError> {
    provider.eth_get_block(anchor.espace_block()).await?.ok_or(
        CoreSpaceContextError::EspaceBlockNotFound {
            epoch_number: context.epoch_number,
            pivot_hash: context.pivot_hash,
        },
    )
}

async fn load_consensus_context(
    provider: &ConfluxSimulationProvider,
    context: CoreSpaceBlockContext,
    pivot_block: &CoreSpaceRpcBlock,
) -> Result<ConsensusContext, CoreSpaceContextError> {
    let Some(pos_reference) = pivot_block.pos_reference else {
        return Ok(ConsensusContext::default());
    };
    let public_pos_reference = b256_from_cfx(pos_reference);
    let pos_block = provider.pos_get_block_by_hash(pos_reference).await?.ok_or(
        CoreSpaceContextError::ConsensusContextUnavailable {
            pivot_hash: context.pivot_hash,
            pos_reference: public_pos_reference,
        },
    )?;

    let finalized_epoch = pos_block
        .pivot_decision
        .ok_or(CoreSpaceContextError::ConsensusContextUnavailable {
            pivot_hash: context.pivot_hash,
            pos_reference: public_pos_reference,
        })?
        .height
        .as_u64();

    Ok(ConsensusContext {
        pos_view: Some(pos_block.height.as_u64()),
        finalized_epoch: Some(finalized_epoch),
    })
}

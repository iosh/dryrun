use std::fmt;

use alloy::{eips::BlockId, primitives::B256};
use cfx_types::U256;
use thiserror::Error;

use crate::{
    ConfluxRpcError,
    execution::{
        ExecutionBlockContext, ExecutionBlockContextError, ExecutionConsensusContext,
        build_core_space_pivot_block_context, build_espace_execution_block_context,
        build_execution_block_context,
    },
    primitive::{b256_from_cfx, b256_to_cfx},
    state::{ConfluxSimulationProvider, ConfluxStateAnchor, CoreSpaceRpcBlock, EspaceRpcBlock},
};

/// Selects the eSpace block used by one simulation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EspaceBlockSelector {
    /// Resolves the endpoint's latest eSpace block once.
    Latest,
    /// Resolves the eSpace block at the given number.
    Number(u64),
    /// Resolves the eSpace block with the given hash.
    Hash(B256),
}

impl EspaceBlockSelector {
    fn block_id(self) -> BlockId {
        match self {
            Self::Latest => BlockId::latest(),
            Self::Number(number) => BlockId::number(number),
            Self::Hash(hash) => BlockId::hash(hash),
        }
    }
}

impl fmt::Display for EspaceBlockSelector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest => formatter.write_str("latest"),
            Self::Number(number) => write!(formatter, "number {number}"),
            Self::Hash(hash) => write!(formatter, "hash {hash:#x}"),
        }
    }
}

/// The immutable cross-space block identity used by an eSpace simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EspaceBlockContext {
    /// The resolved eSpace block number.
    pub number: u64,
    /// The resolved eSpace block hash.
    pub hash: B256,
    /// The epoch number of the dependent Core Space pivot.
    pub core_space_epoch_number: u64,
    /// The hash of the dependent Core Space pivot.
    pub core_space_pivot_hash: B256,
}

/// An error resolving the fixed context for an eSpace simulation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EspaceContextError {
    /// The eSpace block request failed.
    #[error("failed to fetch eSpace block selected by {selector}: {source}")]
    EspaceBlockRequest {
        /// The selector being resolved.
        selector: EspaceBlockSelector,
        /// The underlying RPC failure.
        #[source]
        source: ConfluxRpcError,
    },

    /// The selected eSpace block does not exist.
    #[error("eSpace block selected by {selector} was not found")]
    EspaceBlockNotFound {
        /// The selector that did not resolve.
        selector: EspaceBlockSelector,
    },

    /// The endpoint returned a block that does not satisfy the selector.
    #[error(
        "eSpace block selected by {selector} returned number {actual_number} and hash {actual_hash:#x}"
    )]
    EspaceSelectorMismatch {
        /// The selector requested from the endpoint.
        selector: EspaceBlockSelector,
        /// The number returned by the endpoint.
        actual_number: u64,
        /// The hash returned by the endpoint.
        actual_hash: B256,
    },

    /// The dependent Core Space pivot request failed.
    #[error("failed to fetch Core Space pivot at epoch {epoch_number}: {source}")]
    CoreSpacePivotRequest {
        /// The fixed epoch number being resolved.
        epoch_number: u64,
        /// The underlying RPC failure.
        #[source]
        source: ConfluxRpcError,
    },

    /// The dependent Core Space pivot does not exist.
    #[error("Core Space pivot at epoch {epoch_number} was not found")]
    CoreSpacePivotNotFound {
        /// The fixed epoch number that did not resolve.
        epoch_number: u64,
    },

    /// The Core Space pivot does not match the eSpace state anchor.
    #[error(
        "Core Space pivot does not match the eSpace anchor: expected epoch {expected_epoch_number} and hash {expected_pivot_hash:#x}, got epoch {actual_epoch_number} and hash {actual_pivot_hash:#x}"
    )]
    CoreSpacePivotMismatch {
        /// The epoch fixed from the eSpace block.
        expected_epoch_number: u64,
        /// The pivot hash fixed from the eSpace block.
        expected_pivot_hash: B256,
        /// The epoch returned by the Core Space endpoint.
        actual_epoch_number: u64,
        /// The pivot hash returned by the Core Space endpoint.
        actual_pivot_hash: B256,
    },

    /// The resolved blocks cannot form an execution context.
    #[error("invalid eSpace execution block context: {source}")]
    ExecutionBlockContext {
        /// The invalid execution-context detail.
        #[source]
        source: ExecutionBlockContextError,
    },
}

pub(crate) struct ResolvedEspaceContext {
    pub(crate) execution_block_context: ExecutionBlockContext,
    pub(crate) state_anchor: ConfluxStateAnchor,
    pub(crate) public_context: EspaceBlockContext,
}

impl ResolvedEspaceContext {
    pub(crate) fn base_fee_per_gas(&self) -> Option<U256> {
        self.execution_block_context
            .base_fees
            .espace_base_fee_per_gas
    }

    pub(crate) fn state_block(&self) -> BlockId {
        self.state_anchor.espace_block()
    }
}

pub(crate) async fn resolve_espace_context(
    provider: &ConfluxSimulationProvider,
    selector: EspaceBlockSelector,
) -> Result<ResolvedEspaceContext, EspaceContextError> {
    let espace_block = load_espace_block(provider, selector).await?;
    validate_espace_selector(selector, &espace_block)?;

    let state_anchor = ConfluxStateAnchor::new(espace_block.number, b256_to_cfx(espace_block.hash));
    let core_space_pivot_block =
        load_core_space_pivot(provider, state_anchor.epoch_number()).await?;
    let core_space_pivot = build_validated_core_space_pivot(state_anchor, &core_space_pivot_block)?;

    let public_context = EspaceBlockContext {
        number: espace_block.number,
        hash: espace_block.hash,
        core_space_epoch_number: core_space_pivot.epoch_height,
        core_space_pivot_hash: b256_from_cfx(core_space_pivot.hash),
    };
    let espace_execution_block = build_espace_execution_block_context(&espace_block);
    let execution_block_context = build_execution_block_context(
        &core_space_pivot,
        &espace_execution_block,
        ExecutionConsensusContext::default(),
    );

    Ok(ResolvedEspaceContext {
        execution_block_context,
        state_anchor,
        public_context,
    })
}

async fn load_espace_block(
    provider: &ConfluxSimulationProvider,
    selector: EspaceBlockSelector,
) -> Result<EspaceRpcBlock, EspaceContextError> {
    provider
        .eth_get_block(selector.block_id())
        .await
        .map_err(|source| EspaceContextError::EspaceBlockRequest { selector, source })?
        .ok_or(EspaceContextError::EspaceBlockNotFound { selector })
}

async fn load_core_space_pivot(
    provider: &ConfluxSimulationProvider,
    epoch_number: u64,
) -> Result<CoreSpaceRpcBlock, EspaceContextError> {
    provider
        .cfx_get_block_by_epoch_number(cfx_rpc_cfx_types::EpochNumber::Num(epoch_number.into()))
        .await
        .map_err(|source| EspaceContextError::CoreSpacePivotRequest {
            epoch_number,
            source,
        })?
        .ok_or(EspaceContextError::CoreSpacePivotNotFound { epoch_number })
}

fn validate_espace_selector(
    selector: EspaceBlockSelector,
    block: &EspaceRpcBlock,
) -> Result<(), EspaceContextError> {
    let matches = match selector {
        EspaceBlockSelector::Latest => true,
        EspaceBlockSelector::Number(number) => block.number == number,
        EspaceBlockSelector::Hash(hash) => block.hash == hash,
    };

    if !matches {
        return Err(EspaceContextError::EspaceSelectorMismatch {
            selector,
            actual_number: block.number,
            actual_hash: block.hash,
        });
    }

    Ok(())
}

fn build_validated_core_space_pivot(
    expected: ConfluxStateAnchor,
    block: &CoreSpaceRpcBlock,
) -> Result<crate::execution::CoreSpacePivotBlockContext, EspaceContextError> {
    let actual = build_core_space_pivot_block_context(block)
        .map_err(|source| EspaceContextError::ExecutionBlockContext { source })?;
    let actual_anchor = ConfluxStateAnchor::new(actual.epoch_height, actual.hash);
    if actual_anchor != expected {
        return Err(EspaceContextError::CoreSpacePivotMismatch {
            expected_epoch_number: expected.epoch_number(),
            expected_pivot_hash: b256_from_cfx(expected.pivot_hash()),
            actual_epoch_number: actual_anchor.epoch_number(),
            actual_pivot_hash: b256_from_cfx(actual_anchor.pivot_hash()),
        });
    }

    Ok(actual)
}

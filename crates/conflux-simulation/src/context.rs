use crate::{Error, primitive::*};
use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    primitives::{B256, U256 as AlloyU256},
    providers::{DynProvider, Provider},
    rpc::types::Header,
};
use cfx_executor::{machine::Machine, spec::CommonParams, state::State};
use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use cfx_types::{Address, SpaceMap, U256};
use cfx_vm_types::Env;
use conflux_provider::{ConfluxProvider, CoreRpcBlock, EpochNumber};
use primitives::SignedTransaction;

/// The state identity shared by preparation, execution and analysis.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Anchor {
    pub epoch: u64,
    pub pivot_hash: B256,
}

impl Anchor {
    fn from_pivot(pivot: &CoreRpcBlock) -> Result<Self, Error> {
        Ok(Self {
            epoch: quantity(pivot.epoch_number, "epochNumber")?,
            pivot_hash: pivot.hash,
        })
    }

    pub fn block_id(self) -> BlockId {
        BlockId::hash_canonical(self.pivot_hash)
    }

    pub fn epoch_number(self) -> EpochNumber {
        EpochNumber::Number(self.epoch)
    }

    pub async fn check_pivot(self, core: &ConfluxProvider) -> Result<(), Error> {
        let pivot = core
            .cfx_get_block_by_epoch_number(self.epoch_number(), false)
            .await?;
        if pivot.is_none_or(|block| block.hash != self.pivot_hash) {
            return Err(Error::ContextInconsistent);
        }
        Ok(())
    }
}

/// A fixed state anchor and the execution environment derived from it.
pub(crate) struct BlockContext {
    pub anchor: Anchor,
    pub number: u64,
    pub epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub base_gas_price: SpaceMap<U256>,
    pos_view: Option<u64>,
    finalized_epoch: Option<u64>,
}

impl BlockContext {
    pub async fn fetch_epoch(
        core: &ConfluxProvider,
        espace: &DynProvider,
        epoch: EpochNumber,
        params: &CommonParams,
    ) -> Result<Self, Error> {
        if !matches!(epoch, EpochNumber::Number(_) | EpochNumber::LatestState) {
            return Err(Error::Unsupported(format!(
                "epoch {epoch} is not supported"
            )));
        }
        let pivot = core
            .cfx_get_block_by_epoch_number(epoch, false)
            .await?
            .ok_or(Error::EpochNotFound(epoch))?;
        let anchor = Anchor::from_pivot(&pivot)?;
        if matches!(epoch, EpochNumber::Number(expected) if expected != anchor.epoch) {
            return Err(Error::ContextInconsistent);
        }
        let header = fetch_espace_header(espace, anchor.block_id()).await?;
        let mut context = Self::from_blocks(anchor, &pivot, &header, params)?;
        context.load_pos(core, pivot.pos_reference).await?;
        Ok(context)
    }

    pub async fn fetch_block(
        core: &ConfluxProvider,
        espace: &DynProvider,
        block: BlockId,
        params: &CommonParams,
    ) -> Result<Self, Error> {
        if matches!(
            block,
            BlockId::Number(BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest)
        ) {
            return Err(Error::Unsupported(format!(
                "block {block} is not supported"
            )));
        }
        let header = fetch_espace_header(espace, block).await?;
        let pivot = core
            .cfx_get_block_by_epoch_number(EpochNumber::Number(header.number), false)
            .await?
            .ok_or(Error::BlockNotFound(block))?;
        let anchor = Anchor::from_pivot(&pivot)?;
        Self::from_blocks(anchor, &pivot, &header, params)
    }

    fn from_blocks(
        anchor: Anchor,
        pivot: &CoreRpcBlock,
        espace: &Header,
        params: &CommonParams,
    ) -> Result<Self, Error> {
        if espace.hash != anchor.pivot_hash || espace.number != anchor.epoch {
            return Err(Error::ContextInconsistent);
        }
        let number = quantity(pivot.block_number, "blockNumber")?
            .checked_add(1)
            .ok_or_else(|| Error::Unsupported("blockNumber overflow".into()))?;
        let epoch_height = anchor
            .epoch
            .checked_add(1)
            .ok_or_else(|| Error::Unsupported("epochNumber overflow".into()))?;
        let activation = params.transition_heights.cip1559;
        let base_gas_price = if epoch_height < activation {
            SpaceMap::new(U256::zero(), U256::zero())
        } else if epoch_height == activation {
            params.init_base_price()
        } else {
            SpaceMap::new(
                u256_to_cfx(
                    pivot.base_fee_per_gas.ok_or_else(|| {
                        Error::Unsupported("Core block lacks baseFeePerGas".into())
                    })?,
                ),
                espace
                    .base_fee_per_gas
                    .ok_or_else(|| Error::Unsupported("eSpace block lacks baseFeePerGas".into()))?
                    .into(),
            )
        };
        Ok(Self {
            anchor,
            number,
            epoch_height,
            author: Address::from(pivot.miner.bytes()),
            timestamp: quantity(Some(pivot.timestamp), "timestamp")?,
            base_gas_price,
            pos_view: None,
            finalized_epoch: None,
        })
    }

    async fn load_pos(
        &mut self,
        core: &ConfluxProvider,
        reference: Option<B256>,
    ) -> Result<(), Error> {
        let Some(hash) = reference else {
            return Ok(());
        };
        let block = core.pos_get_block_by_hash(hash).await?.ok_or_else(|| {
            crate::StateError::Unavailable("referenced PoS block is missing".into())
        })?;
        if block.hash != hash {
            return Err(Error::ContextInconsistent);
        }
        let decision = block.pivot_decision.ok_or_else(|| {
            crate::StateError::Unavailable("PoS pivot decision is missing".into())
        })?;
        self.pos_view = Some(quantity(Some(block.height), "PoS height")?);
        self.finalized_epoch = Some(quantity(Some(decision.height), "finalized epoch")?);
        Ok(())
    }

    pub fn env(&self, machine: &Machine, state: &State, tx: &SignedTransaction) -> Env {
        Env {
            chain_id: machine.params().chain_id_map(self.epoch_height),
            number: self.number,
            epoch_height: self.epoch_height,
            author: self.author,
            timestamp: self.timestamp,
            difficulty: U256::zero(),
            accumulated_gas_used: U256::zero(),
            gas_limit: *tx.gas(),
            last_hash: b256_to_cfx(self.anchor.pivot_hash),
            pos_view: self.pos_view,
            finalized_epoch: self.finalized_epoch,
            transaction_epoch_bound: TRANSACTION_DEFAULT_EPOCH_BOUND,
            base_gas_price: self.base_gas_price,
            burnt_gas_price: self
                .base_gas_price
                .map_all(|price| state.burnt_gas_price(price)),
            transaction_hash: tx.hash(),
        }
    }
}

async fn fetch_espace_header(espace: &DynProvider, block: BlockId) -> Result<Header, Error> {
    Ok(espace
        .get_block(block)
        .await
        .map_err(|source| Error::EspaceProvider {
            operation: "eth_getBlock",
            source,
        })?
        .ok_or(Error::BlockNotFound(block))?
        .header)
}

fn quantity(value: Option<AlloyU256>, field: &str) -> Result<u64, Error> {
    value
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| Error::Unsupported(format!("pivot {field} is missing or outside u64")))
}

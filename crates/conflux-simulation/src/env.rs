use crate::{Error, primitive::*};
use alloy::{primitives::U256 as AlloyU256, rpc::types::Header};
use cfx_executor::{machine::Machine, spec::CommonParams, state::State};
use cfx_parameters::consensus::TRANSACTION_DEFAULT_EPOCH_BOUND;
use cfx_types::{Address, H256, SpaceMap, U256};
use cfx_vm_types::Env;
use conflux_provider::CoreRpcBlock;
use primitives::SignedTransaction;

/// Execution facts derived once from the selected pivot and epoch.
#[derive(Clone)]
pub(crate) struct BlockContext {
    pub number: u64,
    pub epoch_height: u64,
    pub author: Address,
    pub timestamp: u64,
    pub hash: H256,
    pub base_gas_price: SpaceMap<U256>,
}

impl BlockContext {
    pub fn new(
        pivot: &CoreRpcBlock,
        espace: &Header,
        params: &CommonParams,
    ) -> Result<Self, Error> {
        let number = quantity(pivot.block_number, "blockNumber")?
            .checked_add(1)
            .ok_or_else(|| Error::Unsupported("blockNumber overflow".into()))?;
        let epoch = quantity(pivot.epoch_number, "epochNumber")?;
        if epoch != espace.number {
            return Err(Error::ContextInconsistent);
        }
        let epoch_height = epoch
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
            number,
            epoch_height,
            author: Address::from(pivot.miner.bytes()),
            timestamp: quantity(Some(pivot.timestamp), "timestamp")?,
            hash: b256_to_cfx(pivot.hash),
            base_gas_price,
        })
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
            last_hash: self.hash,
            pos_view: None,
            finalized_epoch: None,
            transaction_epoch_bound: TRANSACTION_DEFAULT_EPOCH_BOUND,
            base_gas_price: self.base_gas_price,
            burnt_gas_price: self
                .base_gas_price
                .map_all(|price| state.burnt_gas_price(price)),
            transaction_hash: tx.hash(),
        }
    }
}

fn quantity(value: Option<AlloyU256>, field: &str) -> Result<u64, Error> {
    value
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| Error::Unsupported(format!("pivot {field} is missing or outside u64")))
}

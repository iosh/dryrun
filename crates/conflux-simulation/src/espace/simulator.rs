use super::{execution, transaction};
use crate::{ChainSpec, Error, context::BlockContext, state::StateSource};
use alloy::{
    eips::{BlockId, BlockNumHash},
    primitives::{Address, U256},
    providers::DynProvider,
    rpc::types::TransactionRequest,
};
use cfx_executor::machine::Machine;
use cfx_types::Space;
use conflux_provider::ConfluxProvider;
use serde::Serialize;
use simulation_core::Outcome;
use simulation_core::{ExecutionStatus, Limits};
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct Simulator {
    core: ConfluxProvider,
    espace: DynProvider,
    chain: ChainSpec,
    machine: Arc<Machine>,
    limits: Limits,
}

#[derive(Debug, Clone)]
pub struct SimulationRequest {
    pub block: BlockId,
    pub transaction: TransactionRequest,
}

#[derive(Debug, Serialize)]
pub struct Simulation {
    #[serde(serialize_with = "simulation_core::serialize_block")]
    pub block: BlockNumHash,
    /// The transaction as executed, with omitted fields filled in.
    pub transaction: TransactionRequest,
    pub outcome: Outcome<Execution>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    #[serde(with = "alloy_serde::quantity")]
    pub gas_used: u64,
    #[serde(with = "alloy_serde::quantity")]
    pub gas_charged: u64,
    pub fee: Fee,
    #[serde(flatten)]
    pub status: ExecutionStatus<simulation_core::ChangeSet<Address>, Error>,
}

/// The transaction fee. Native balance changes exclude it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fee {
    /// Price paid per gas.
    #[serde(with = "alloy_serde::quantity")]
    pub gas_price: u128,
    /// Base fee of the simulated block.
    #[serde(with = "alloy_serde::quantity")]
    pub base_fee: u128,
    /// Total paid by the sender.
    pub amount: U256,
}

impl Simulator {
    pub async fn new(
        core: ConfluxProvider,
        espace: DynProvider,
        chain: ChainSpec,
        limits: Limits,
    ) -> Result<Self, Error> {
        crate::endpoint::check_network(&chain, &core, &espace).await?;
        let machine = Arc::new(chain.machine());
        Ok(Self {
            core,
            espace,
            chain,
            machine,
            limits,
        })
    }

    pub async fn simulate(&self, request: SimulationRequest) -> Result<Simulation, Error> {
        let preparation = transaction::Preparation::new(request.transaction)?;
        let context =
            BlockContext::fetch_block(&self.core, &self.espace, request.block, &self.chain.params)
                .await?;
        let anchor = context.anchor;
        let source = Arc::new(
            StateSource::new(
                anchor,
                self.core.clone(),
                self.espace.clone(),
                self.chain.network,
                self.limits,
            )
            .await?,
        );
        let (transaction, tx) = preparation
            .complete(
                &context,
                &source,
                self.chain
                    .params
                    .chain_id(context.epoch_height, Space::Ethereum),
                context.epoch_height >= self.chain.params.transition_heights.cip1559,
            )
            .await?;
        anchor.check_pivot(&self.core).await?;
        let runtime = Handle::current();
        let machine = Arc::clone(&self.machine);
        let result = tokio::task::spawn_blocking(move || {
            execution::execute(source, runtime, &machine, context, tx)
        })
        .await;
        // Every formal execution outcome passes the same final pivot check,
        // including rejection, VM failure and unavailable changes.
        anchor.check_pivot(&self.core).await?;
        let outcome = result.map_err(Error::Runtime)??;
        Ok(Simulation {
            block: BlockNumHash::new(anchor.epoch, anchor.pivot_hash),
            transaction,
            outcome,
        })
    }
}

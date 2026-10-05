use alloy::{
    primitives::{B256, U256},
    providers::DynProvider,
};
use cfx_executor::machine::Machine;
use cfx_types::Space;
use conflux_provider::{ConfluxProvider, EpochNumber};
use serde::Serialize;
use simulation_core::{ExecutionStatus, Limits, Outcome};
use std::sync::Arc;
use tokio::runtime::Handle;

use super::{Address, Changes, TransactionRequest, execution, transaction};
use crate::{ChainSpec, Error, context::BlockContext, state::StateSource};

pub struct Simulator {
    core: ConfluxProvider,
    espace: DynProvider,
    chain: ChainSpec,
    machine: Arc<Machine>,
    limits: Limits,
}

#[derive(Debug, Clone)]
pub struct SimulationRequest {
    pub epoch: EpochNumber,
    pub transaction: TransactionRequest,
}

#[derive(Debug, Serialize)]
pub struct Simulation {
    pub epoch: Epoch,
    pub transaction: TransactionRequest,
    pub approximate: bool,
    pub limitations: &'static [&'static str],
    pub outcome: Outcome<Execution>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Epoch {
    #[serde(with = "alloy_serde::quantity")]
    pub number: u64,
    pub pivot_hash: B256,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub gas_used: U256,
    pub gas_charged: U256,
    pub fee: Fee,
    #[serde(flatten)]
    pub status: ExecutionStatus<Changes, Error>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fee {
    pub gas_price: U256,
    pub base_fee: U256,
    pub amount: U256,
    pub payer: FeePayer,
}

/// The source of the gas fee, never inferred from sponsor configuration alone.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FeePayer {
    Sender {
        address: Address,
    },
    /// The contract's gas sponsor pool, not its ordinary balance.
    Sponsor {
        contract: Address,
    },
}

const LIMITATIONS: &[&str] = &[
    "storageOwnershipApproximated",
    "storageCollateralSkipped",
    "storagePointsApproximated",
    "storagePointsInitializationAssumed",
    "sponsorWhitelistApproximated",
    "contractCleanupPartial",
];

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
        let preparation = transaction::Preparation::new(request.transaction, self.chain.network)?;
        let context =
            BlockContext::fetch_epoch(&self.core, &self.espace, request.epoch, &self.chain.params)
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
                    .chain_id(context.epoch_height, Space::Native),
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
        anchor.check_pivot(&self.core).await?;
        let outcome = result.map_err(Error::Runtime)??;
        Ok(Simulation {
            epoch: Epoch {
                number: anchor.epoch,
                pivot_hash: anchor.pivot_hash,
            },
            transaction,
            approximate: true,
            limitations: LIMITATIONS,
            outcome,
        })
    }
}

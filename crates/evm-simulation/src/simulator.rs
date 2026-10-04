use std::sync::Arc;

use alloy::{
    eips::{BlockId, BlockNumHash},
    primitives::{Address, U256},
    providers::{DynProvider, Provider},
    rpc::types::TransactionRequest,
};
use revm::primitives::hardfork::SpecId;
use serde::{Serialize, Serializer};
use simulation_core::{ExecutionStatus, Limits, Outcome};
use tokio::runtime::Handle;

use crate::{
    ChainSpec, Error, block,
    db::{ForkDatabase, VmDatabase},
    execution, transaction,
};

#[derive(Debug, Clone)]
pub struct Simulator {
    provider: DynProvider,
    chain: Arc<ChainSpec>,
    limits: Limits,
}

#[derive(Debug, Clone)]
pub struct SimulationRequest {
    pub block: BlockId,
    pub transaction: TransactionRequest,
}

#[derive(Debug, Serialize)]
pub struct Simulation {
    #[serde(serialize_with = "serialize_block")]
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
    pub base_fee: u64,
    /// Price paid per blob gas, for blob transactions.
    #[serde(
        with = "alloy_serde::quantity::opt",
        skip_serializing_if = "Option::is_none"
    )]
    pub blob_gas_price: Option<u128>,
    /// Total paid by the sender.
    pub amount: U256,
}

impl Simulator {
    /// Fails if the provider serves a different chain.
    pub async fn new(
        provider: DynProvider,
        chain: ChainSpec,
        limits: Limits,
    ) -> Result<Self, Error> {
        let actual = provider
            .get_chain_id()
            .await
            .map_err(|source| Error::Provider {
                operation: "eth_chainId",
                source,
            })?;
        if actual != chain.chain_id {
            return Err(Error::ChainMismatch {
                expected: chain.chain_id,
                actual,
            });
        }
        Ok(Self {
            provider,
            chain: Arc::new(chain),
            limits,
        })
    }

    pub async fn simulate(&self, request: SimulationRequest) -> Result<Simulation, Error> {
        let SimulationRequest { block, transaction } = request;
        let preparation = transaction::Preparation::new(transaction)?;
        let header = block::fetch_header(&self.provider, block).await?;
        let block = BlockNumHash::new(header.number, header.hash);
        let env = self.chain.evm_env(&header.inner);
        // Before EIP-161 the VM tells an existing empty account from a missing
        // one, which the provider's account reads cannot.
        if !env.cfg_env.spec.is_enabled_in(SpecId::SPURIOUS_DRAGON) {
            return Err(Error::Unsupported(
                "blocks before Spurious Dragon are not supported".into(),
            ));
        }
        let mut db = ForkDatabase::new(self.provider.clone(), &header, self.limits);
        let (transaction, tx) = preparation.complete(&env, &mut db).await?;
        let budget = Arc::clone(db.budget());
        let runtime = Handle::current();

        tokio::task::spawn_blocking(move || {
            let db = VmDatabase::new(db, runtime);
            let outcome = execution::execute(&db, &env, tx, &budget)?;
            Ok(Simulation {
                block,
                transaction,
                outcome,
            })
        })
        .await
        .map_err(Error::Runtime)?
    }
}

fn serialize_block<S: Serializer>(block: &BlockNumHash, serializer: S) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Block {
        #[serde(with = "alloy_serde::quantity")]
        number: u64,
        hash: alloy::primitives::B256,
    }

    Block {
        number: block.number,
        hash: block.hash,
    }
    .serialize(serializer)
}

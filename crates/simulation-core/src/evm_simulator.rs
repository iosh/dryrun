use crate::error::{SimulationError, SimulationResult};
use alloy::{
    eips::BlockId,
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
};
use revm::{
    Context, ExecuteCommitEvm, MainBuilder, MainContext,
    context::{BlockEnv, CfgEnv, Transaction, TxEnv, result::ExecutionResult},
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    interpreter::Host,
    primitives::{Bytes, eip7825, hardfork::SpecId},
};
use types::{EvmSimulateInput, EvmSimulateOutput};

type AlloyCacheDB = CacheDB<WrapDatabaseAsync<AlloyDB<Ethereum, DynProvider>>>;

pub struct EvmSimulator {
    provider: DynProvider,
}

impl EvmSimulator {
    pub fn new(rpc_url: &str) -> Self {
        let provider: DynProvider = ProviderBuilder::new()
            .connect_http(rpc_url.parse().unwrap())
            .erased();
        EvmSimulator { provider }
    }

    pub async fn simulate(&self, input: EvmSimulateInput) -> SimulationResult<EvmSimulateOutput> {
        let db = self.create_database(&input.block_id).await?;

        // TODO add account state overrides
        let cfg_env: CfgEnv<SpecId> = CfgEnv::default();

        // TODO add block overrides
        let block_env = BlockEnv::default();

        let tx = &input.transaction;
        let tx_env = TxEnv::builder()
            .caller(tx.from.unwrap_or_default())
            .value(tx.value.unwrap_or_default())
            .data(tx.input.clone().data.unwrap_or_default())
            .gas_limit(tx.gas.unwrap_or(eip7825::TX_GAS_LIMIT_CAP))
            .gas_price(tx.gas_price.unwrap_or_default())
            .nonce(tx.nonce.unwrap_or_default())
            .build()
            .map_err(SimulationError::InvalidTransaction)?;

        let mut evm = Context::mainnet()
            .with_db(db)
            .with_cfg(cfg_env)
            .with_block(block_env)
            .build_mainnet();

        let result = evm.transact_commit(tx_env)?;

        let (status, logs) = match result {
            ExecutionResult::Success { ref logs, .. } => (true, logs),
            _ => (false, &vec![]),
        };

        let output = EvmSimulateOutput {
            status,
            gas_used: result.gas_used(),
            block_number: evm.block_number(),
            logs: logs.to_vec(),
        };

        Ok(output)
    }

    async fn create_database(&self, block_id: &BlockId) -> SimulationResult<AlloyCacheDB> {
        let alloy_db =
            WrapDatabaseAsync::new(AlloyDB::new(self.provider.clone(), block_id.clone())).unwrap();
        Ok(CacheDB::new(alloy_db))
    }
}

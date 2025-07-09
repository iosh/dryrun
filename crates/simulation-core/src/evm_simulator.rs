use crate::error::{SimulationError, SimulationResult};
use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::{Block, TransactionRequest},
};
use revm::{
    Context, ExecuteCommitEvm, MainBuilder, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv, result::ExecutionResult},
    context_interface::block::BlobExcessGasAndPrice,
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    interpreter::Host,
    primitives::{U256, eip4844::BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE, eip7825},
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
        let block_id = input
            .block_id
            .unwrap_or(BlockId::Number(BlockNumberOrTag::Latest));

        let execution_block = self
            .provider
            .get_block(block_id)
            .await?
            .ok_or(SimulationError::BlockNumberNotFound)?;

        // TODO add block overrides
        let mut block_env = self.build_block_env(&execution_block);

        let db = self.create_database(&block_id).await?;

        let chain_id = self.provider.get_chain_id().await?;
        // TODO add account state overrides
        let cfg_env = self.build_cfg_env(chain_id);

        let tx_env = self.build_tx_env(&input.transaction, block_env.basefee as u128)?;

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

    fn build_tx_env(&self, tx: &TransactionRequest, base_fee: u128) -> SimulationResult<TxEnv> {
        let mut tx_builder = TxEnv::builder()
            .caller(tx.from.unwrap_or_default())
            .value(tx.value.unwrap_or_default())
            .data(tx.input.clone().data.unwrap_or_default())
            .gas_limit(tx.gas.unwrap_or(eip7825::TX_GAS_LIMIT_CAP))
            .gas_price(tx.gas_price.unwrap_or_default())
            .nonce(tx.nonce.unwrap_or_default());

        if let Some(to) = tx.to {
            tx_builder = tx_builder.kind(to)
        }

        if let Some(max_fee_per_gas) = tx.max_fee_per_gas {
            tx_builder = tx_builder
                .max_fee_per_gas(max_fee_per_gas)
                .gas_priority_fee(tx.max_priority_fee_per_gas);
        } else {
            let gas_price = tx.gas_price.unwrap_or(base_fee);
            tx_builder = tx_builder.gas_price(gas_price);
        }

        let tx_env = tx_builder
            .build()
            .map_err(SimulationError::InvalidTransaction)?;

        Ok(tx_env)
    }

    fn build_cfg_env(&self, chain_id: u64) -> CfgEnv {
        CfgEnv::default().with_chain_id(chain_id)
    }

    fn build_block_env(&self, execution_block: &Block) -> BlockEnv {
        let block_number = execution_block.number();

        let blob_excess_gas_and_price = execution_block
            .header
            .excess_blob_gas
            .map(|excess| BlobExcessGasAndPrice::new(excess, BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE));

        BlockEnv {
            number: U256::from(block_number),
            beneficiary: execution_block.header.beneficiary,
            timestamp: U256::from(execution_block.header.timestamp),
            difficulty: execution_block.header.difficulty(),
            prevrandao: execution_block.header.mix_hash(),
            gas_limit: execution_block.header.gas_limit,
            basefee: execution_block.header.base_fee_per_gas.unwrap_or_default(),
            blob_excess_gas_and_price,
        }
    }

    async fn create_database(&self, block_id: &BlockId) -> SimulationResult<AlloyCacheDB> {
        let alloy_db =
            WrapDatabaseAsync::new(AlloyDB::new(self.provider.clone(), *block_id)).unwrap();
        Ok(CacheDB::new(alloy_db))
    }
}

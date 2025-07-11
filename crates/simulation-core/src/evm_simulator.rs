use crate::error::{SimulationError, SimulationResult};
use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::{Block, BlockOverrides, TransactionRequest, state::StateOverride},
};
use revm::{
    Context, Database, DatabaseCommit, ExecuteCommitEvm, MainBuilder, MainContext,
    context::{BlockEnv, CfgEnv, TxEnv, result::ExecutionResult},
    context_interface::block::BlobExcessGasAndPrice,
    database::{AlloyDB, CacheDB, WrapDatabaseAsync},
    interpreter::Host,
    primitives::{
        HashMap, U256, eip4844::BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE, eip7825, keccak256,
    },
    state::{Account, AccountStatus, Bytecode, EvmStorageSlot},
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

        let mut db = self.create_database(&block_id).await?;

        let mut block_env = self.build_block_env(&execution_block);

        if let Some(overrides) = input.block_overrides {
            self.apply_block_overrides(&mut block_env, &mut db, overrides);
        }

        if let Some(state_overrides) = input.state_overrides {
            self.apply_state_overrides(&mut db, state_overrides)?;
        }

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

    fn apply_block_overrides(
        &self,
        block_env: &mut BlockEnv,
        db: &mut AlloyCacheDB,
        block_overrides: BlockOverrides,
    ) {
        if let Some(block_hashes) = block_overrides.block_hash {
            db.cache.block_hashes.extend(
                block_hashes
                    .into_iter()
                    .map(|(num, hash)| (U256::from(num), hash)),
            );
        }

        if let Some(number) = block_overrides.number {
            block_env.number = number.saturating_to();
        }

        if let Some(difficulty) = block_overrides.difficulty {
            block_env.difficulty = difficulty;
        }

        if let Some(time) = block_overrides.time {
            block_env.timestamp = U256::from(time);
        }

        if let Some(gas_limit) = block_overrides.gas_limit {
            block_env.gas_limit = gas_limit;
        }

        if let Some(coinbase) = block_overrides.coinbase {
            block_env.beneficiary = coinbase;
        }

        if let Some(random) = block_overrides.random {
            block_env.prevrandao = Some(random);
        }

        if let Some(base_fee) = block_overrides.base_fee {
            block_env.basefee = base_fee.saturating_to();
        }
    }

    fn apply_state_overrides(
        &self,
        db: &mut AlloyCacheDB,
        state_overrides: StateOverride,
    ) -> SimulationResult<()> {
        for (account, account_override) in state_overrides {
            let mut info = db.basic(account)?.unwrap_or_default();

            if let Some(nonce) = account_override.nonce {
                info.nonce = nonce;
            }

            if let Some(code) = account_override.code {
                info.code_hash = keccak256(&code);
                info.code = Some(Bytecode::new_raw_checked(code)?);
            }

            if let Some(balance) = account_override.balance {
                info.balance = balance;
            }

            let mut acc = revm::state::Account {
                info,
                status: AccountStatus::Touched,
                storage: Default::default(),
                transaction_id: 0,
            };

            let storage_diff = match (account_override.state, account_override.state_diff) {
                (Some(_), Some(_)) => {
                    return Err(SimulationError::BothStateAndStateDiff(account));
                }
                (None, None) => None,

                (Some(state), None) => {
                    db.commit(HashMap::from_iter([(
                        account,
                        Account {
                            status: AccountStatus::SelfDestructed | AccountStatus::Touched,
                            ..Default::default()
                        },
                    )]));
                    acc.mark_created();
                    Some(state)
                }
                (None, Some(state)) => Some(state),
            };

            if let Some(state) = storage_diff {
                for (slot, value) in state {
                    acc.storage.insert(
                        slot.into(),
                        EvmStorageSlot {
                            original_value: (!value).into(),
                            present_value: value.into(),
                            is_cold: false,
                            transaction_id: 0,
                        },
                    );
                }
            }
            db.commit(HashMap::from_iter([(account, acc)]));
        }

        Ok(())
    }

    async fn create_database(&self, block_id: &BlockId) -> SimulationResult<AlloyCacheDB> {
        let alloy_db =
            WrapDatabaseAsync::new(AlloyDB::new(self.provider.clone(), *block_id)).unwrap();
        Ok(CacheDB::new(alloy_db))
    }
}

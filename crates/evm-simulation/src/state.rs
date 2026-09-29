use alloy::primitives::{Address, Bytes, TxKind};
use alloy_evm::EvmEnv;
use revm::{
    Context, ExecuteEvm, MainBuilder, MainContext,
    context::{Cfg, CfgEnv, TxEnv},
    context_interface::result::{EVMError, ExecutionResult},
    database_interface::{DatabaseRef, WrapDatabaseRef},
};
use simulation_core::{CallResult, ReadBudget, StateView};

use crate::{Error, StateError};

/// Read-only calls on one state. Each call runs in a fresh VM on top of `db`,
/// and its writes are dropped with the VM.
pub(crate) struct StateReader<'a, DB> {
    db: &'a DB,
    env: EvmEnv,
    budget: &'a ReadBudget,
}

impl<'a, DB: DatabaseRef<Error = StateError>> StateReader<'a, DB> {
    pub(crate) fn new(db: &'a DB, env: &EvmEnv, budget: &'a ReadBudget) -> Self {
        let mut cfg: CfgEnv = env.cfg_env.clone();
        cfg.tx_chain_id_check = false;
        cfg.disable_nonce_check = true;
        cfg.disable_balance_check = true;
        cfg.disable_base_fee = true;
        cfg.disable_fee_charge = true;
        cfg.disable_eip3607 = true;
        Self {
            db,
            env: EvmEnv::new(cfg, env.block_env.clone()),
            budget,
        }
    }
}

impl<DB: DatabaseRef<Error = StateError>> StateView<Address> for StateReader<'_, DB> {
    type Error = Error;

    fn call(&self, contract: Address, input: Bytes) -> Result<CallResult, Error> {
        self.budget.record_read_call()?;
        let gas_limit = self
            .budget
            .limits()
            .read_call_gas
            .min(self.env.cfg_env.tx_gas_limit_cap());
        let tx = TxEnv {
            caller: Address::ZERO,
            gas_limit,
            kind: TxKind::Call(contract),
            data: input,
            ..Default::default()
        };
        let mut evm = Context::mainnet()
            .with_db(WrapDatabaseRef(self.db))
            .with_cfg(self.env.cfg_env.clone())
            .with_block(self.env.block_env.clone())
            .build_mainnet();
        let result = match evm.transact(tx) {
            Ok(result) => match result.result {
                ExecutionResult::Success { output, .. } => CallResult::Success(output.into_data()),
                ExecutionResult::Revert { .. } => CallResult::Revert,
                ExecutionResult::Halt { .. } => CallResult::Halt,
            },
            Err(EVMError::Database(error)) => return Err(error.into()),
            Err(error) => return Err(Error::Execution(format!("read call: {error}"))),
        };
        if let CallResult::Success(output) = &result {
            self.budget.check_read_call_output(output.len())?;
        }
        Ok(result)
    }
}

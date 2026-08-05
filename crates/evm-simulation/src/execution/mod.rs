mod chain_spec;
mod env;
mod fee_settlement;
mod observation;
mod state;

use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    primitives::Address,
};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext, MainnetEvm as RevmMainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        result::{EVMError, ExecutionResult, HaltReason, InvalidTransaction},
        transaction::Transaction,
    },
    handler::EvmTr,
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
};
use simulation_transaction::Transaction as SimulationTransaction;
use thiserror::Error;

use self::{
    chain_spec::resolve_execution_spec_id,
    env::{create_block_env, create_cfg_env, create_tx_env},
};

pub type MainnetEvmDatabase = state::MainnetEvmDatabase;
pub type MainnetEvm<INSP = ()> = MainnetEvmWithDatabase<MainnetEvmDatabase, INSP>;
pub(crate) type MainnetEvmWithDb<DB, INSP = ()> =
    RevmMainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB>, INSP>;
type MainnetEvmWithDatabase<DB, INSP = ()> = MainnetEvmWithDb<DB, INSP>;

pub use observation::{EvmExecutionObservation, EvmExecutionObserver};

#[derive(Debug, Error)]
pub enum EvmExecutionError {
    #[error("only Ethereum mainnet is supported now, got chain_id={0}")]
    UnsupportedChain(u64),

    #[error("hardfork {0} is not mapped to revm::SpecId yet")]
    UnsupportedHardfork(String),

    #[error("block context is invalid: {0}")]
    BlockContext(String),

    #[error("state access failed during execution: {0}")]
    StateAccess(String),

    #[error("EVM execution failed: {0}")]
    Execution(String),

    #[error("transaction was not executed: {0}")]
    NotExecuted(InvalidTransaction),

    #[error("transaction fee settlement arithmetic was inconsistent")]
    FeeSettlement,

    #[error("transaction transition has already been applied")]
    TransitionAlreadyApplied,

    #[error("transaction transition is only applicable to a successful execution")]
    TransitionNotApplicable,
}

#[derive(Debug)]
pub struct EvmExecutionOutput<INSP> {
    result: ExecutionResult<HaltReason>,
    transition: Option<EvmState>,
    fee_settlement: EvmFeeSettlement,
    evm: MainnetEvm<INSP>,
}

impl<INSP> EvmExecutionOutput<INSP> {
    pub fn result(&self) -> &ExecutionResult<HaltReason> {
        &self.result
    }

    pub fn transition(&self) -> Result<&EvmState, EvmExecutionError> {
        self.transition
            .as_ref()
            .ok_or(EvmExecutionError::TransitionAlreadyApplied)
    }

    pub fn fee_settlement(&self) -> &EvmFeeSettlement {
        &self.fee_settlement
    }

    pub fn caller(&self) -> Address {
        self.evm.ctx_ref().tx.caller
    }

    pub fn beneficiary(&self) -> Address {
        self.evm.ctx_ref().block.beneficiary
    }

    pub fn evm_mut(&mut self) -> &mut MainnetEvm<INSP> {
        &mut self.evm
    }

    pub fn take_inspector(&mut self) -> INSP
    where
        INSP: Default,
    {
        std::mem::take(&mut self.evm.inspector)
    }

    pub fn apply_transition(&mut self) -> Result<(), EvmExecutionError> {
        if !self.result.is_success() {
            return Err(EvmExecutionError::TransitionNotApplicable);
        }

        let transition = self
            .transition
            .take()
            .ok_or(EvmExecutionError::TransitionAlreadyApplied)?;
        self.evm.commit(transition);
        Ok(())
    }
}

impl EvmExecutionOutput<EvmExecutionObserver> {
    pub fn observations(&self) -> Vec<EvmExecutionObservation> {
        self.evm.inspector.observations()
    }
}

#[derive(Debug)]
pub struct EvmTransactionExecutor<INSP> {
    evm: MainnetEvm<INSP>,
}

impl<INSP> EvmTransactionExecutor<INSP> {
    pub fn new(
        state_source: EvmStateSource,
        block: Sealed<Header>,
        chain_id: u64,
        inspector: INSP,
    ) -> Result<Self, EvmExecutionError> {
        let block_anchor = EvmBlockAnchor::new(block.number(), block.hash());
        if state_source.anchor() != block_anchor {
            return Err(EvmExecutionError::BlockContext(format!(
                "state source is anchored at block {} ({}) but execution uses block {} ({})",
                state_source.anchor().number(),
                state_source.anchor().hash(),
                block_anchor.number(),
                block_anchor.hash(),
            )));
        }

        let spec_id = resolve_execution_spec_id(chain_id, block.number(), block.timestamp())?;
        let cfg_env = create_cfg_env(chain_id, spec_id);
        let block_env = create_block_env(block.inner(), spec_id)?;
        let evm = Context::mainnet()
            .with_db(state_source.into_database())
            .modify_cfg_chained(|cfg| *cfg = cfg_env)
            .modify_block_chained(|current_block| *current_block = block_env)
            .build_mainnet_with_inspector(inspector);

        Ok(Self { evm })
    }

    pub fn execute(
        mut self,
        transaction: &SimulationTransaction,
    ) -> Result<EvmExecutionOutput<INSP>, EvmExecutionError>
    where
        INSP: revm::Inspector<Context<BlockEnv, TxEnv, CfgEnv, MainnetEvmDatabase>, EthInterpreter>,
    {
        let tx_env = create_tx_env(transaction);
        let effective_gas_price = tx_env.effective_gas_price(self.evm.ctx().block.basefee as u128);
        let base_fee_per_gas = self.evm.ctx().block.basefee;
        let result_and_state = match self.evm.inspect_tx(tx_env) {
            Ok(result_and_state) => result_and_state,
            Err(EVMError::Transaction(error)) => {
                return Err(EvmExecutionError::NotExecuted(error));
            }
            Err(EVMError::Header(error)) => {
                return Err(EvmExecutionError::BlockContext(format!(
                    "EVM header validation failed: {error}"
                )));
            }
            Err(EVMError::Database(error)) => {
                return Err(EvmExecutionError::StateAccess(format!(
                    "state access failed during execution: {error}"
                )));
            }
            Err(EVMError::Custom(error)) => {
                return Err(EvmExecutionError::Execution(format!(
                    "EVM execution failed: {error}"
                )));
            }
        };

        let fee_settlement = EvmFeeSettlement::new(
            &result_and_state.result,
            effective_gas_price,
            base_fee_per_gas,
        )?;

        Ok(EvmExecutionOutput {
            result: result_and_state.result,
            transition: Some(result_and_state.state),
            fee_settlement,
            evm: self.evm,
        })
    }
}

pub use fee_settlement::EvmFeeSettlement;
pub use state::{EvmBlockAnchor, EvmStateSource};

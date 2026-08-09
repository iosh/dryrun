mod env;
mod fee_settlement;
mod observation;
mod outcome_mapping;
mod rejection_mapping;
mod state;

use self::{
    env::{create_block_env, create_cfg_env, create_tx_env},
    rejection_mapping::map_transaction_rejection,
};
use crate::{
    CompleteTransaction, CompleteTransactionVariant, EthereumChainSpec, EvmBlobGasFee,
    EvmBlockEnvironmentError, EvmExecutionError, EvmExecutionResult, EvmGas, EvmNotReadyError,
    EvmSimulationError, EvmStateAccessError, EvmTransactionRejection,
};
use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    primitives::{Address, Log},
};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext, MainnetEvm as RevmMainnetEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        result::{EVMError, ExecutionResult, HaltReason, InvalidHeader},
        transaction::Transaction,
    },
    handler::EvmTr,
    interpreter::interpreter::EthInterpreter,
    primitives::eip4844::GAS_PER_BLOB,
    state::EvmState,
};

pub(crate) type MainnetEvmDatabase = state::MainnetEvmDatabase;
pub(crate) type MainnetEvm<INSP = ()> = MainnetEvmWithDb<MainnetEvmDatabase, INSP>;
pub(crate) type MainnetEvmWithDb<DB, INSP = ()> =
    RevmMainnetEvm<Context<BlockEnv, TxEnv, CfgEnv, DB>, INSP>;

pub(crate) use observation::{EvmExecutionObservation, EvmExecutionObserver};
pub(crate) use outcome_mapping::map_executed_outcome;

#[derive(Debug)]
pub(crate) enum EvmTransactionExecution<INSP> {
    Executed(Box<ExecutedTransaction<INSP>>),
    NotExecuted(EvmTransactionRejection),
}

#[derive(Debug)]
pub(crate) struct ExecutedTransaction<INSP> {
    result: ExecutionResult<HaltReason>,
    gas: EvmGas,
    transition: Option<EvmState>,
    fee_settlement: EvmFeeSettlement,
    evm: MainnetEvm<INSP>,
}

impl<INSP> ExecutedTransaction<INSP> {
    pub(crate) fn is_success(&self) -> bool {
        self.result.is_success()
    }

    pub(crate) fn into_outcome_parts(self) -> (ExecutionResult<HaltReason>, EvmExecutionResult) {
        let Self {
            result,
            gas,
            fee_settlement,
            ..
        } = self;
        let execution_result = EvmExecutionResult::new(gas, fee_settlement.into_fee());

        (result, execution_result)
    }

    pub(crate) fn transition(&self) -> Result<&EvmState, EvmExecutionError> {
        self.transition
            .as_ref()
            .ok_or(EvmExecutionError::TransitionAlreadyApplied)
    }

    pub(crate) fn committed_logs(&self) -> &[Log] {
        self.result.logs()
    }

    pub(crate) const fn fee_settlement(&self) -> &EvmFeeSettlement {
        &self.fee_settlement
    }

    pub(crate) fn caller(&self) -> Address {
        self.evm.ctx_ref().tx.caller
    }

    pub(crate) fn beneficiary(&self) -> Address {
        self.evm.ctx_ref().block.beneficiary
    }

    pub(crate) const fn evm_mut(&mut self) -> &mut MainnetEvm<INSP> {
        &mut self.evm
    }

    pub(crate) fn apply_transition(&mut self) -> Result<(), EvmExecutionError> {
        let transition = self
            .transition
            .take()
            .ok_or(EvmExecutionError::TransitionAlreadyApplied)?;
        self.evm.commit(transition);
        Ok(())
    }
}

impl ExecutedTransaction<EvmExecutionObserver> {
    pub(crate) fn take_observations(&mut self) -> Vec<EvmExecutionObservation> {
        self.evm.inspector.take_observations()
    }
}

#[derive(Debug)]
pub(crate) struct EvmTransactionExecutor<INSP> {
    evm: MainnetEvm<INSP>,
    chain_id: u64,
    block_number: u64,
    block_gas_limit: u64,
    base_fee_per_gas: u64,
    blob_gas_price: Option<u128>,
}

impl<INSP> EvmTransactionExecutor<INSP> {
    pub(crate) fn new(
        database: MainnetEvmDatabase,
        block: Sealed<Header>,
        chain_spec: &EthereumChainSpec,
        inspector: INSP,
    ) -> Result<Self, EvmSimulationError> {
        let block_number = block.number();
        let spec_id = chain_spec
            .execution_spec_id(block_number, block.timestamp())
            .map_err(EvmNotReadyError::from)?;
        let chain_id = chain_spec.chain_id();
        let cfg_env = create_cfg_env(chain_id, spec_id);
        let block_env =
            create_block_env(block.inner(), spec_id).map_err(EvmExecutionError::from)?;
        let block_gas_limit = block_env.gas_limit;
        let base_fee_per_gas = block_env.basefee;
        let blob_gas_price = block_env
            .blob_excess_gas_and_price
            .as_ref()
            .map(|blob| blob.blob_gasprice);
        let evm = Context::mainnet()
            .with_db(database)
            .modify_cfg_chained(|cfg| *cfg = cfg_env)
            .modify_block_chained(|current_block| *current_block = block_env)
            .build_mainnet_with_inspector(inspector);

        Ok(Self {
            evm,
            chain_id,
            block_number,
            block_gas_limit,
            base_fee_per_gas,
            blob_gas_price,
        })
    }

    pub(crate) fn execute(
        mut self,
        transaction: &CompleteTransaction,
    ) -> Result<EvmTransactionExecution<INSP>, EvmSimulationError>
    where
        INSP: revm::Inspector<Context<BlockEnv, TxEnv, CfgEnv, MainnetEvmDatabase>, EthInterpreter>,
    {
        let tx_env = create_tx_env(transaction);
        let effective_gas_price = tx_env.effective_gas_price(self.base_fee_per_gas as u128);
        let result_and_state = match self.evm.inspect_tx(tx_env) {
            Ok(result_and_state) => result_and_state,
            Err(EVMError::Transaction(error)) => {
                let rejection = map_transaction_rejection(
                    error,
                    transaction,
                    self.chain_id,
                    self.block_gas_limit,
                    self.base_fee_per_gas,
                )?;
                return Ok(EvmTransactionExecution::NotExecuted(rejection));
            }
            Err(EVMError::Header(error)) => {
                return Err(
                    EvmExecutionError::from(map_header_error(error, self.block_number)).into(),
                );
            }
            Err(EVMError::Database(error)) => {
                return Err(EvmExecutionError::from(EvmStateAccessError::from(error)).into());
            }
            Err(EVMError::Custom(details)) => {
                return Err(EvmExecutionError::engine_failure(details).into());
            }
        };

        let result_gas = result_and_state.result.gas();
        let gas = EvmGas::new(
            transaction.gas_limit,
            result_gas.limit(),
            result_gas.intrinsic_gas(),
            result_gas.spent(),
            result_gas.inner_refunded(),
            result_gas.floor_gas(),
        )
        .map_err(EvmExecutionError::from)?;
        let blob_gas_fee = match &transaction.variant {
            CompleteTransactionVariant::Eip4844 {
                blob_versioned_hashes,
                ..
            } => {
                let gas_price = self
                    .blob_gas_price
                    .ok_or(EvmBlockEnvironmentError::MissingExcessBlobGas {
                        block_number: self.block_number,
                    })
                    .map_err(EvmExecutionError::from)?;
                Some(EvmBlobGasFee::new(
                    GAS_PER_BLOB * blob_versioned_hashes.len() as u64,
                    gas_price,
                ))
            }
            CompleteTransactionVariant::Legacy { .. }
            | CompleteTransactionVariant::Eip2930 { .. }
            | CompleteTransactionVariant::Eip1559 { .. }
            | CompleteTransactionVariant::Eip7702 { .. } => None,
        };
        let fee_settlement = EvmFeeSettlement::new(
            &gas,
            transaction.gas_limit,
            effective_gas_price,
            self.base_fee_per_gas,
            blob_gas_fee,
        )
        .map_err(EvmExecutionError::from)?;

        Ok(EvmTransactionExecution::Executed(Box::new(
            ExecutedTransaction {
                result: result_and_state.result,
                gas,
                transition: Some(result_and_state.state),
                fee_settlement,
                evm: self.evm,
            },
        )))
    }
}

const fn map_header_error(error: InvalidHeader, block_number: u64) -> EvmBlockEnvironmentError {
    match error {
        InvalidHeader::PrevrandaoNotSet => {
            EvmBlockEnvironmentError::MissingPrevRandao { block_number }
        }
        InvalidHeader::ExcessBlobGasNotSet => {
            EvmBlockEnvironmentError::MissingExcessBlobGas { block_number }
        }
    }
}

pub(crate) use fee_settlement::EvmFeeSettlement;
pub(crate) use state::create_database;

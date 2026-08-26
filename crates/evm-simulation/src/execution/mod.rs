mod env;
mod events;
mod fee_settlement;
mod outcome_mapping;
mod rejection_mapping;

use self::{
    env::{create_block_env, create_cfg_env, create_tx_env},
    rejection_mapping::map_transaction_rejection,
};
use crate::{
    CompleteTransaction, CompleteTransactionVariant, EthereumChainSpec, EvmBlobGasFee,
    EvmBlockEnvironmentError, EvmExecutionError, EvmExecutionResult, EvmGas, EvmNotReadyError,
    EvmSimulationError, EvmStateAccessError, EvmTransactionRejection,
    state::{EvmDatabase, EvmStateView, MainnetEvm},
};
use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    primitives::{Address, Log},
};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext,
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

pub(crate) use events::{EvmExecutionEvent, EvmExecutionObserver};
pub(crate) use outcome_mapping::map_executed_outcome;

#[derive(Debug)]
pub(crate) enum EvmTransactionExecutionResult<INSP> {
    Executed(Box<ExecutedTransaction<INSP>>),
    NotExecuted(EvmTransactionRejection),
}

#[derive(Debug)]
pub(crate) struct ExecutedTransaction<INSP> {
    result: ExecutionResult<HaltReason>,
    gas: EvmGas,
    transition: EvmState,
    fee_settlement: EvmFeeSettlement,
    evm: MainnetEvm<INSP>,
}

impl ExecutedTransaction<EvmExecutionObserver> {
    pub(crate) fn commit(
        mut self,
        transaction: &CompleteTransaction,
    ) -> (EvmTransactionExecution, EvmStateView) {
        let events = self.evm.inspector.take_events();
        let caller = self.evm.ctx_ref().tx.caller;
        let beneficiary = self.evm.ctx_ref().block.beneficiary;
        self.evm.commit(self.transition.clone());

        let state_view = EvmStateView::from_execution(self.evm, transaction);
        let execution = EvmTransactionExecution {
            result: self.result,
            gas: self.gas,
            transition: self.transition,
            fee_settlement: self.fee_settlement,
            caller,
            beneficiary,
            events,
        };

        (execution, state_view)
    }
}

pub(crate) struct EvmTransactionExecution {
    result: ExecutionResult<HaltReason>,
    gas: EvmGas,
    transition: EvmState,
    fee_settlement: EvmFeeSettlement,
    caller: Address,
    beneficiary: Address,
    events: Vec<EvmExecutionEvent>,
}

impl EvmTransactionExecution {
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
        (
            result,
            EvmExecutionResult::new(gas, fee_settlement.into_fee()),
        )
    }

    pub(crate) fn transition(&self) -> &EvmState {
        &self.transition
    }

    pub(crate) fn committed_logs(&self) -> &[Log] {
        self.result.logs()
    }

    pub(crate) const fn fee_settlement(&self) -> &EvmFeeSettlement {
        &self.fee_settlement
    }

    pub(crate) fn caller(&self) -> Address {
        self.caller
    }

    pub(crate) fn beneficiary(&self) -> Address {
        self.beneficiary
    }

    pub(crate) fn events(&self) -> &[EvmExecutionEvent] {
        &self.events
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
        database: EvmDatabase,
        block: Sealed<Header>,
        chain_spec: &EthereumChainSpec,
        inspector: INSP,
    ) -> Result<Self, EvmSimulationError> {
        let block_number = block.number();
        let execution_spec = chain_spec
            .execution_spec(block_number, block.timestamp())
            .map_err(EvmNotReadyError::from)?;
        let chain_id = chain_spec.chain_id();
        let cfg_env = create_cfg_env(chain_id, execution_spec);
        let block_env =
            create_block_env(block.inner(), execution_spec).map_err(EvmExecutionError::from)?;
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
    ) -> Result<EvmTransactionExecutionResult<INSP>, EvmSimulationError>
    where
        INSP: revm::Inspector<Context<BlockEnv, TxEnv, CfgEnv, EvmDatabase>, EthInterpreter>,
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
                return Ok(EvmTransactionExecutionResult::NotExecuted(rejection));
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

        Ok(EvmTransactionExecutionResult::Executed(Box::new(
            ExecutedTransaction {
                result: result_and_state.result,
                gas,
                transition: result_and_state.state,
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

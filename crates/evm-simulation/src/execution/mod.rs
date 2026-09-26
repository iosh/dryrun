mod env;
mod events;
mod fee_settlement;
mod native;
mod outcome_mapping;
mod rejection_mapping;

use simulation_core::observation::AnalysisLimitExceeded;

use self::{
    env::{create_block_env, create_cfg_env, create_tx_env},
    outcome_mapping::{EvmFinalStatus, map_executed_status},
    rejection_mapping::map_transaction_rejection,
};
use crate::{
    EthereumChainSpec, EvmBlobGasFee, EvmBlockEnvironmentError, EvmExecutionError,
    EvmExecutionOutcome, EvmExecutionResult, EvmGas, EvmNotReadyError, EvmResultIntegrationError,
    EvmSimulationError, EvmSimulationLimits, EvmStateAccessError, EvmTransactionRejection,
    TypedTransaction,
    state::{
        EvmDatabase, EvmStateAccess, EvmStateAccessFactory, EvmStateReader, EvmStateSource,
        MainnetEvm,
    },
};
use alloy::{
    consensus::{BlockHeader, Header, Sealed},
    primitives::{Address, Log},
};
use revm::{
    Context, InspectEvm,
    context::{BlockEnv, CfgEnv, TxEnv},
    context_interface::{
        ContextTr,
        result::{EVMError, ExecutionResult, HaltReason, InvalidHeader},
        transaction::Transaction,
    },
    handler::EvmTr,
    interpreter::interpreter::EthInterpreter,
    primitives::{eip4844::GAS_PER_BLOB, hardfork::SpecId},
    state::EvmState,
};

pub(crate) use events::EvmExecutionObserver;
pub use events::{
    EvmCallKind, EvmCommittedFrame, EvmCommittedLog, EvmCommittedSelfdestruct,
    EvmExecutionPosition, EvmFrameAction, EvmFrameId, EvmStorageWrite,
};
pub(crate) use native::NativeMovement;

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
    state_access_factory: EvmStateAccessFactory,
    read_call_caller: Address,
    block_beneficiary: Address,
}

impl ExecutedTransaction<EvmExecutionObserver> {
    pub(crate) fn commit(
        mut self,
        transaction: &TypedTransaction,
    ) -> Result<EvmTransactionExecution, EvmExecutionError> {
        let observation = self.evm.inspector.take_observation().map_err(|error| {
            EvmResultIntegrationError::new(format!("execution observation: {error}"))
        })?;
        if observation.limit_exceeded.is_none() {
            verify_committed_logs(&observation.logs, self.result.logs())?;
        }
        verify_committed_create_addresses(&observation.frames)?;
        let native_movements = native::collect_movements(&observation);
        let status = map_executed_status(self.result, transaction)?;

        let anchor_cache = self.evm.ctx().db().cache.clone();
        let events::EvmExecutionObservation {
            applied_authorization_accounts,
            frames,
            logs,
            selfdestructs,
            storage_writes,
            checkpoints,
            limit_exceeded,
        } = observation;
        let state = EvmStateAccess::new(
            self.state_access_factory,
            anchor_cache,
            self.read_call_caller,
            if limit_exceeded.is_none() {
                checkpoints
            } else {
                Vec::new()
            },
            self.transition,
        );
        let execution = EvmTransactionExecution {
            status,
            gas: self.gas,
            fee_settlement: self.fee_settlement,
            fee_payer: self.read_call_caller,
            block_beneficiary: self.block_beneficiary,
            applied_authorization_accounts,
            committed_frames: frames,
            committed_logs: logs,
            committed_selfdestructs: selfdestructs,
            storage_writes,
            state,
            limit_exceeded,
            native_movements,
        };

        Ok(execution)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EvmLogCheckpoint<'a> {
    committed_log: &'a EvmCommittedLog,
    previous_state: &'a EvmStateReader,
    state: &'a EvmStateReader,
}

impl<'a> EvmLogCheckpoint<'a> {
    pub const fn position(self) -> EvmExecutionPosition {
        self.committed_log.position()
    }

    pub const fn frame_id(self) -> EvmFrameId {
        self.committed_log.frame_id()
    }

    pub const fn log(self) -> &'a Log {
        self.committed_log.log()
    }

    /// The preceding retained checkpoint, or the transaction's initial state.
    pub const fn previous_state(self) -> &'a EvmStateReader {
        self.previous_state
    }

    /// Persistent state at the log emission point.
    pub const fn state(self) -> &'a EvmStateReader {
        self.state
    }
}

#[derive(Debug)]
pub struct EvmTransactionExecution {
    status: EvmFinalStatus,
    gas: EvmGas,
    fee_settlement: EvmFeeSettlement,
    fee_payer: Address,
    block_beneficiary: Address,
    native_movements: Vec<NativeMovement>,
    applied_authorization_accounts: Vec<Address>,
    committed_frames: Vec<EvmCommittedFrame>,
    committed_logs: Vec<EvmCommittedLog>,
    committed_selfdestructs: Vec<EvmCommittedSelfdestruct>,
    storage_writes: Vec<EvmStorageWrite>,
    state: EvmStateAccess,
    limit_exceeded: Option<AnalysisLimitExceeded>,
}

impl EvmTransactionExecution {
    pub(crate) fn native_movements(&self) -> &[NativeMovement] {
        &self.native_movements
    }

    pub fn storage_writes(&self) -> &[EvmStorageWrite] {
        &self.storage_writes
    }

    pub fn is_success(&self) -> bool {
        matches!(self.status, EvmFinalStatus::Success { .. })
    }

    pub(crate) fn into_outcome(self) -> EvmExecutionOutcome {
        let Self {
            status,
            gas,
            fee_settlement,
            ..
        } = self;
        let result = EvmExecutionResult::new(gas, fee_settlement.into_fee());
        match status {
            EvmFinalStatus::Success {
                reason,
                output,
                logs,
            } => EvmExecutionOutcome::Success {
                result,
                reason,
                output,
                logs,
            },
            EvmFinalStatus::Reverted {
                revert_data,
                reason,
            } => EvmExecutionOutcome::Reverted {
                result,
                revert_data,
                reason,
            },
            EvmFinalStatus::Halted { reason } => EvmExecutionOutcome::Halted { result, reason },
        }
    }

    pub fn fee_payer(&self) -> Address {
        self.fee_payer
    }

    pub fn block_beneficiary(&self) -> Address {
        self.block_beneficiary
    }

    pub fn fee(&self) -> &crate::EvmFee {
        self.fee_settlement.fee()
    }

    pub fn applied_authorization_accounts(&self) -> &[Address] {
        &self.applied_authorization_accounts
    }

    pub fn committed_frames(&self) -> &[EvmCommittedFrame] {
        &self.committed_frames
    }

    pub fn committed_logs(&self) -> &[EvmCommittedLog] {
        &self.committed_logs
    }

    pub fn committed_selfdestructs(&self) -> &[EvmCommittedSelfdestruct] {
        &self.committed_selfdestructs
    }

    pub(crate) fn state(&self) -> &EvmStateAccess {
        &self.state
    }

    pub(crate) fn check_observation_limit(&self) -> Result<(), AnalysisLimitExceeded> {
        self.limit_exceeded.map_or(Ok(()), Err)
    }

    pub(crate) fn log_checkpoints(&self) -> impl Iterator<Item = EvmLogCheckpoint<'_>> {
        self.state
            .log_checkpoints()
            .map(|(log_index, previous_state, state)| EvmLogCheckpoint {
                committed_log: &self.committed_logs[log_index],
                previous_state,
                state,
            })
    }
}

fn verify_committed_logs(
    observed: &[EvmCommittedLog],
    result: &[Log],
) -> Result<(), EvmResultIntegrationError> {
    if observed.len() != result.len() {
        return Err(EvmResultIntegrationError::new(format!(
            "observer retained {} committed logs, but the execution result returned {}",
            observed.len(),
            result.len()
        )));
    }
    if let Some((index, _)) = observed
        .iter()
        .zip(result)
        .enumerate()
        .find(|(_, (observed, result))| observed.log() != *result)
    {
        return Err(EvmResultIntegrationError::new(format!(
            "observer log at committed index {index} differs from the execution result"
        )));
    }
    Ok(())
}

fn verify_committed_create_addresses(
    frames: &[EvmCommittedFrame],
) -> Result<(), EvmResultIntegrationError> {
    if frames.iter().any(|frame| {
        matches!(
            frame.action(),
            EvmFrameAction::Create {
                created_address: None,
                ..
            }
        )
    }) {
        return Err(EvmResultIntegrationError::new(
            "successful contract creation did not return the created address",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct EvmTransactionExecutor<INSP> {
    evm: MainnetEvm<INSP>,
    state_access_factory: EvmStateAccessFactory,
    chain_id: u64,
    block_number: u64,
    block_gas_limit: u64,
    base_fee_per_gas: u64,
    burn_enabled: bool,
    blob_gas_price: Option<u128>,
    block_beneficiary: Address,
}

impl<INSP> EvmTransactionExecutor<INSP> {
    pub(crate) fn new(
        state_source: EvmStateSource,
        block: Sealed<Header>,
        chain_spec: &EthereumChainSpec,
        inspector: INSP,
        limits: EvmSimulationLimits,
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
        let burn_enabled = execution_spec.spec_id.is_enabled_in(SpecId::LONDON);
        let blob_gas_price = block_env
            .blob_excess_gas_and_price
            .as_ref()
            .map(|blob| blob.blob_gasprice);
        let block_beneficiary = block_env.beneficiary;
        let state_access_factory =
            EvmStateAccessFactory::with_limits(state_source, cfg_env, block_env, limits);
        let evm = state_access_factory.create_execution_evm(inspector);

        Ok(Self {
            evm,
            state_access_factory,
            chain_id,
            block_number,
            block_gas_limit,
            base_fee_per_gas,
            burn_enabled,
            blob_gas_price,
            block_beneficiary,
        })
    }

    pub(crate) fn execute(
        mut self,
        transaction: &TypedTransaction,
    ) -> Result<EvmTransactionExecutionResult<INSP>, EvmSimulationError>
    where
        INSP: revm::Inspector<Context<BlockEnv, TxEnv, CfgEnv, EvmDatabase>, EthInterpreter>,
    {
        let tx_env = create_tx_env(transaction)?;
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
        let common = transaction.common();
        let gas = EvmGas::new(
            common.gas_limit,
            result_gas.limit(),
            result_gas.intrinsic_gas(),
            result_gas.spent(),
            result_gas.inner_refunded(),
            result_gas.floor_gas(),
        )
        .map_err(EvmExecutionError::from)?;
        let blob_gas_fee = match transaction {
            TypedTransaction::Eip4844 {
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
            TypedTransaction::Legacy { .. }
            | TypedTransaction::Eip2930 { .. }
            | TypedTransaction::Eip1559 { .. }
            | TypedTransaction::Eip7702 { .. } => None,
        };
        let fee_settlement = EvmFeeSettlement::new(
            &gas,
            effective_gas_price,
            self.base_fee_per_gas,
            self.burn_enabled,
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
                state_access_factory: self.state_access_factory,
                read_call_caller: common.from,
                block_beneficiary: self.block_beneficiary,
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

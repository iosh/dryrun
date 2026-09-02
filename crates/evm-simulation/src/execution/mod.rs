mod env;
mod events;
mod fee_settlement;
mod outcome_mapping;
mod rejection_mapping;

use std::sync::Arc;

use self::{
    env::{create_block_env, create_cfg_env, create_tx_env},
    rejection_mapping::map_transaction_rejection,
};
use crate::{
    CompleteTransaction, EthereumChainSpec, EvmBlobGasFee, EvmBlockEnvironmentError,
    EvmExecutionError, EvmExecutionResult, EvmGas, EvmNotReadyError, EvmResultIntegrationError,
    EvmSimulationError, EvmSimulationLimits, EvmStateAccessError, EvmTransactionRejection,
    state::{
        EvmDatabase, EvmExecutionIdentity, EvmOccurrenceHandle, EvmStateSource,
        EvmStateViewFactory, EvmStateViews, MainnetEvm,
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
    EvmCallKind, EvmCommittedFrame, EvmCommittedFrameKind, EvmCommittedLog,
    EvmCommittedSelfdestruct, EvmExecutionPosition, EvmFrameId, EvmOccurrenceEvidenceError,
};
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
    state_view_factory: EvmStateViewFactory,
    read_call_caller: Address,
    block_beneficiary: Address,
}

impl ExecutedTransaction<EvmExecutionObserver> {
    pub(crate) fn commit(
        mut self,
    ) -> Result<(EvmTransactionExecution, EvmStateViews), EvmExecutionError> {
        let observation =
            self.evm.inspector.take_observation().map_err(|error| {
                EvmResultIntegrationError::execution_observation(error.to_string())
            })?;
        verify_committed_logs(&observation.logs, self.result.logs())?;
        verify_committed_create_addresses(&observation.frames)?;

        let anchor_cache = self.evm.ctx().db().cache.clone();
        let identity = Arc::new(EvmExecutionIdentity);
        let events::EvmExecutionObservation {
            applied_authorization_accounts,
            frames,
            logs,
            selfdestructs,
            semantic_logs,
            checkpoints,
            evidence_error,
        } = observation;
        let semantic_log_occurrences = match evidence_error {
            Some(error) => Err(error),
            None => Ok(semantic_logs
                .into_iter()
                .map(|semantic_log| {
                    let checkpoint_index = semantic_log.checkpoint_index.ok_or_else(|| {
                        EvmResultIntegrationError::execution_observation(format!(
                            "semantic log at index {} has no occurrence checkpoint",
                            semantic_log.log_index
                        ))
                    })?;
                    if checkpoint_index >= checkpoints.len() {
                        return Err(EvmResultIntegrationError::execution_observation(format!(
                            "semantic log at index {} references missing occurrence checkpoint {}",
                            semantic_log.log_index, checkpoint_index
                        )));
                    }
                    let committed_log =
                        logs.get(semantic_log.log_index).cloned().ok_or_else(|| {
                            EvmResultIntegrationError::execution_observation(format!(
                                "semantic log at index {} has no retained log",
                                semantic_log.log_index
                            ))
                        })?;
                    Ok(EvmSemanticLogOccurrence {
                        committed_log,
                        handle: EvmOccurrenceHandle::new(Arc::clone(&identity), checkpoint_index),
                    })
                })
                .collect::<Result<Vec<_>, EvmResultIntegrationError>>()?),
        };
        let occurrence_states = if semantic_log_occurrences.is_ok() {
            checkpoints
        } else {
            Vec::new()
        };
        let state_views = EvmStateViews::new(
            self.state_view_factory,
            anchor_cache,
            Arc::clone(&identity),
            self.read_call_caller,
            occurrence_states,
            self.transition,
        );
        let execution = EvmTransactionExecution {
            result: self.result,
            gas: self.gas,
            fee_settlement: self.fee_settlement,
            fee_payer: self.read_call_caller,
            block_beneficiary: self.block_beneficiary,
            applied_authorization_accounts,
            committed_frames: frames,
            committed_logs: logs,
            committed_selfdestructs: selfdestructs,
            semantic_log_occurrences,
        };

        Ok((execution, state_views))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmSemanticLogOccurrence {
    committed_log: EvmCommittedLog,
    handle: EvmOccurrenceHandle,
}

impl EvmSemanticLogOccurrence {
    pub const fn position(&self) -> EvmExecutionPosition {
        self.committed_log.position()
    }

    pub const fn frame_id(&self) -> EvmFrameId {
        self.committed_log.frame_id()
    }

    pub const fn log(&self) -> &Log {
        self.committed_log.log()
    }

    pub const fn handle(&self) -> &EvmOccurrenceHandle {
        &self.handle
    }
}

#[derive(Debug)]
pub struct EvmTransactionExecution {
    result: ExecutionResult<HaltReason>,
    gas: EvmGas,
    fee_settlement: EvmFeeSettlement,
    fee_payer: Address,
    block_beneficiary: Address,
    applied_authorization_accounts: Vec<Address>,
    committed_frames: Vec<EvmCommittedFrame>,
    committed_logs: Vec<EvmCommittedLog>,
    committed_selfdestructs: Vec<EvmCommittedSelfdestruct>,
    semantic_log_occurrences: Result<Vec<EvmSemanticLogOccurrence>, EvmOccurrenceEvidenceError>,
}

impl EvmTransactionExecution {
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

    pub fn semantic_log_occurrences(
        &self,
    ) -> Result<&[EvmSemanticLogOccurrence], EvmOccurrenceEvidenceError> {
        self.semantic_log_occurrences
            .as_deref()
            .map_err(Clone::clone)
    }
}

fn verify_committed_logs(
    observed: &[EvmCommittedLog],
    result: &[Log],
) -> Result<(), EvmResultIntegrationError> {
    if observed.len() != result.len() {
        return Err(EvmResultIntegrationError::CommittedLogCountMismatch {
            observed: observed.len(),
            result: result.len(),
        });
    }
    if let Some((index, _)) = observed
        .iter()
        .zip(result)
        .enumerate()
        .find(|(_, (observed, result))| observed.log() != *result)
    {
        return Err(EvmResultIntegrationError::CommittedLogMismatch { index });
    }
    Ok(())
}

fn verify_committed_create_addresses(
    frames: &[EvmCommittedFrame],
) -> Result<(), EvmResultIntegrationError> {
    if frames.iter().any(|frame| {
        matches!(
            frame.kind(),
            EvmCommittedFrameKind::Create {
                created_address: None,
                ..
            }
        )
    }) {
        return Err(EvmResultIntegrationError::MissingCreateAddress);
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct EvmTransactionExecutor<INSP> {
    evm: MainnetEvm<INSP>,
    state_view_factory: EvmStateViewFactory,
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
        let state_view_factory =
            EvmStateViewFactory::with_limits(state_source, cfg_env, block_env, limits);
        let evm = state_view_factory.create_execution_evm(inspector);

        Ok(Self {
            evm,
            state_view_factory,
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
            CompleteTransaction::Eip4844 {
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
            CompleteTransaction::Legacy { .. }
            | CompleteTransaction::Eip2930 { .. }
            | CompleteTransaction::Eip1559 { .. }
            | CompleteTransaction::Eip7702 { .. } => None,
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
                state_view_factory: self.state_view_factory,
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

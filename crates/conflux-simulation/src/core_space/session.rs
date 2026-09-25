use std::sync::Arc;

use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend,
    context::ExecutionBlockContext,
    execution::{
        ConfluxTransactionExecutor, DryRunTransactionInput, ExecutionTraceObserver,
        TransactionExecutionInput, build_conflux_state,
    },
    state::ConfluxStateSource,
};

use super::{
    CoreSpaceExecutionError, CoreSpaceExecutionOutcome, CoreSpaceSimulationError,
    CoreSpaceStateAccessError, CoreSpaceTypedTransaction, StorageSponsorship,
    executed_transaction::CoreSpaceExecutedTransaction,
    outcome::{build_execution_outcome, map_drop_error, map_reconsider_packing_error},
    transaction::build_core_space_transaction_input,
};

pub(super) struct CoreSpaceExecutionSession {
    state: State,
    machine: Arc<Machine>,
    chain_id: u32,
    state_source: Arc<ConfluxStateSource>,
    runtime_handle: Handle,
}

pub(super) struct CoreSpaceExecutionEvidence {
    pub(super) outcome: CoreSpaceExecutionOutcome,
    pub(super) record: CoreSpaceExecutedTransaction,
    pub(super) state: super::CoreSpaceStateAccess,
}

impl CoreSpaceExecutionSession {
    pub(super) fn new(
        backend: &ConfluxSimulationBackend,
        state_source: ConfluxStateSource,
        runtime_handle: Handle,
    ) -> Result<Self, CoreSpaceExecutionError> {
        let state_source = Arc::new(state_source);
        let state = build_conflux_state(Arc::clone(&state_source), runtime_handle.clone())
            .map_err(|source| {
                CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Initialization {
                    source,
                })
            })?;

        Ok(Self {
            state,
            machine: Arc::new(backend.chain_spec().build_machine()),
            chain_id: backend.chain_spec().core_space_chain_id(),
            state_source,
            runtime_handle,
        })
    }

    pub(super) fn execute(
        mut self,
        transaction: &CoreSpaceTypedTransaction,
        block_context: ExecutionBlockContext,
        storage_sponsorship: Option<StorageSponsorship>,
    ) -> Result<
        simulation_core::simulation::Execution<
            CoreSpaceExecutionEvidence,
            super::CoreSpaceTransactionRejection,
        >,
        CoreSpaceSimulationError,
    > {
        let execution_input = TransactionExecutionInput {
            block_context,
            transaction: DryRunTransactionInput::CoreSpace(build_core_space_transaction_input(
                transaction,
                self.chain_id,
            )),
        };
        let observer = ExecutionTraceObserver::new(Space::Native);
        let execution = ConfluxTransactionExecutor::new(&mut self.state, &self.machine)
            .execute(execution_input, observer)
            .map_err(map_execution_error)?;

        use simulation_core::simulation::Execution;
        let prepared = execution.prepared;
        let outcome = match execution.outcome {
            crate::execution::ConfluxExecutionOutcome::NotExecutedDrop(error) => {
                return Ok(Execution::Rejected(
                    map_drop_error(error, transaction.common().from.network())
                        .map_err(CoreSpaceExecutionError::from)?,
                ));
            }
            crate::execution::ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
                return Ok(Execution::Rejected(
                    map_reconsider_packing_error(error).map_err(CoreSpaceExecutionError::from)?,
                ));
            }
            outcome => outcome,
        };
        let state = super::CoreSpaceStateAccess::new(
            self.state_source,
            self.runtime_handle,
            self.state,
            Arc::clone(&self.machine),
            &prepared,
            transaction.common().from.network(),
        )
        .map_err(CoreSpaceExecutionError::from)?;
        let record = CoreSpaceExecutedTransaction::from_outcome(
            outcome,
            &prepared,
            &self.machine,
            transaction.common().from,
            transaction.common().to,
        )?;
        let outcome = build_execution_outcome(&record, transaction, &state, storage_sponsorship)?;
        Ok(Execution::Executed(CoreSpaceExecutionEvidence {
            outcome,
            record,
            state,
        }))
    }
}

fn map_execution_error(
    error: crate::execution::TransactionExecutionError,
) -> CoreSpaceExecutionError {
    use crate::execution::TransactionExecutionError;

    match error {
        TransactionExecutionError::StateAccess(source) => {
            CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Operation {
                operation: "execute Core Space transaction",
                source,
            })
        }
        TransactionExecutionError::MissingExecutionTrace => {
            CoreSpaceExecutionError::ResultIntegration(
                super::CoreSpaceResultIntegrationError::MissingExecutionTrace,
            )
        }
        TransactionExecutionError::GasValueOutOfRange { field, value } => {
            CoreSpaceExecutionError::ResultIntegration(
                super::CoreSpaceResultIntegrationError::GasValueOutOfRange {
                    field,
                    value: crate::primitive::u256_from_cfx(value),
                },
            )
        }
    }
}

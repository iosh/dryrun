use std::sync::Arc;

use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend,
    execution::{
        ConfluxTransactionExecutor, DryRunTransactionInput, ExecutionBlockContext,
        ExecutionTraceObserver, TransactionExecutionInput, build_conflux_state,
    },
    state::ConfluxStateSource,
};

use super::{
    CoreSpaceChanges, CoreSpaceCompleteTransaction, CoreSpaceExecutionError,
    CoreSpaceExecutionOutcome, CoreSpaceSimulationError, CoreSpaceStateAccessError,
    ResolvedStorageSponsorship,
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

pub(super) struct CoreSpaceExecutionSessionResult {
    pub(super) outcome: CoreSpaceExecutionOutcome,
    pub(super) changes: CoreSpaceChanges,
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
        transaction: &CoreSpaceCompleteTransaction,
        block_context: ExecutionBlockContext,
        storage_sponsorship: Option<ResolvedStorageSponsorship>,
        change_rules: &impl super::CoreSpaceChangeRules,
    ) -> Result<CoreSpaceExecutionSessionResult, CoreSpaceSimulationError> {
        let execution_input = TransactionExecutionInput {
            block_context,
            transaction: DryRunTransactionInput::CoreSpace(build_core_space_transaction_input(
                transaction,
                self.chain_id,
            )),
        };
        let execution = ConfluxTransactionExecutor::new(&mut self.state, &self.machine)
            .execute(execution_input, ExecutionTraceObserver::new(Space::Native))
            .map_err(map_execution_error)?;

        let prepared = execution.prepared;
        let (outcome, changes) = match execution.outcome {
            crate::execution::ConfluxExecutionOutcome::NotExecutedDrop(error) => (
                CoreSpaceExecutionOutcome::NotExecuted(
                    map_drop_error(error, transaction.common().from.network())
                        .map_err(CoreSpaceExecutionError::from)?,
                ),
                CoreSpaceChanges::Complete(super::CoreSpaceChangeSet::default()),
            ),
            crate::execution::ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(error) => (
                CoreSpaceExecutionOutcome::NotExecuted(
                    map_reconsider_packing_error(error).map_err(CoreSpaceExecutionError::from)?,
                ),
                CoreSpaceChanges::Complete(super::CoreSpaceChangeSet::default()),
            ),
            executor_outcome => {
                let executed = CoreSpaceExecutedTransaction::from_outcome(
                    executor_outcome,
                    &prepared,
                    &self.machine,
                    transaction.common().from,
                    transaction.common().to,
                )?;
                let state_access = super::state_access::CoreSpaceStateAccess::new(
                    Arc::clone(&self.state_source),
                    self.runtime_handle,
                    self.state,
                    Arc::clone(&self.machine),
                    &prepared,
                    transaction.common().from.network(),
                )
                .map_err(CoreSpaceExecutionError::from)?;
                let outcome = build_execution_outcome(
                    &executed,
                    transaction,
                    &state_access,
                    storage_sponsorship,
                )?;
                let changes =
                    CoreSpaceChanges::from(change_rules.derive_changes(&executed, &state_access));
                (outcome, changes)
            }
        };

        Ok(CoreSpaceExecutionSessionResult { outcome, changes })
    }
}

fn map_execution_error(
    error: crate::execution::TransactionExecutionError,
) -> CoreSpaceExecutionError {
    use crate::execution::TransactionExecutionError;

    match error {
        TransactionExecutionError::BlockContext(source) => {
            CoreSpaceExecutionError::Context { source }
        }
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

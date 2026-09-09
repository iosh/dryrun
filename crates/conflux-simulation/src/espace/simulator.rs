use std::sync::Arc;

use cfx_types::Space;
use tokio::runtime::Handle;

use super::{
    EspaceExecutedTransaction, EspaceExecutionError, EspaceExecutionOutcome,
    EspaceResultIntegrationError, EspaceSimulation, EspaceSimulationError, EspaceSimulationLimits,
    EspaceSimulationRequest, EspaceStateAccess, EspaceStateAccessError, build_executor_transaction,
    classify_transaction_rejection, complete_transaction, convert_executor_outcome,
    resolve_espace_context,
};
use crate::{
    ConfluxSimulationBackend,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecutor, DryRunTransactionInput,
        ExecutionTraceObserver, TransactionExecutionInput, build_conflux_state,
        next_execution_block_number, next_execution_epoch_height,
    },
    state::ConfluxStateSource,
};

pub struct EspaceTransactionSimulator<R = super::DefaultEspaceChangeRules> {
    backend: ConfluxSimulationBackend,
    limits: EspaceSimulationLimits,
    change_rules: Arc<R>,
}

impl<R> Clone for EspaceTransactionSimulator<R> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            limits: self.limits,
            change_rules: Arc::clone(&self.change_rules),
        }
    }
}

impl EspaceTransactionSimulator<super::DefaultEspaceChangeRules> {
    pub fn new(backend: ConfluxSimulationBackend, limits: EspaceSimulationLimits) -> Self {
        let change_rules = super::DefaultEspaceChangeRules::new(
            backend.chain_spec().espace_native_currency().clone(),
            backend.chain_spec().espace_wrapped_native_token(),
        );
        Self {
            backend,
            limits,
            change_rules: Arc::new(change_rules),
        }
    }
}

impl<R> EspaceTransactionSimulator<R> {
    pub fn with_change_rules<N>(self, change_rules: N) -> EspaceTransactionSimulator<N>
    where
        N: super::EspaceChangeRules,
    {
        EspaceTransactionSimulator {
            backend: self.backend,
            limits: self.limits,
            change_rules: Arc::new(change_rules),
        }
    }

    pub fn with_additional_change_rules<N>(
        self,
        change_rules: N,
    ) -> EspaceTransactionSimulator<super::CombinedEspaceChangeRules<R, N>>
    where
        R: super::EspaceChangeRules,
        N: super::EspaceChangeRules,
    {
        EspaceTransactionSimulator {
            backend: self.backend,
            limits: self.limits,
            change_rules: Arc::new(super::CombinedEspaceChangeRules::from_shared(
                self.change_rules,
                change_rules,
            )),
        }
    }
}

impl<R> EspaceTransactionSimulator<R>
where
    R: super::EspaceChangeRules,
{
    /// Simulates one eSpace transaction inside the caller's active Tokio runtime.
    pub async fn simulate(
        &self,
        request: EspaceSimulationRequest,
    ) -> Result<EspaceSimulation, EspaceSimulationError> {
        let EspaceSimulationRequest { block, transaction } = request;
        let runtime_handle =
            Handle::try_current().map_err(|_| EspaceSimulationError::RuntimeUnavailable)?;
        let mut context = resolve_espace_context(self.backend.provider(), block).await?;
        let execution_block_number =
            next_execution_block_number(context.execution_block_context.pivot_block_number)
                .map_err(|error| {
                    EspaceSimulationError::Execution(super::EspaceExecutionError::Context {
                        source: error,
                    })
                })?;
        let execution_epoch_height =
            next_execution_epoch_height(context.execution_block_context.pivot_epoch_height)
                .map_err(|error| {
                    EspaceSimulationError::Execution(super::EspaceExecutionError::Context {
                        source: error,
                    })
                })?;
        context
            .execution_block_context
            .resolve_base_fees(
                self.backend.chain_spec().common_params(),
                execution_epoch_height,
            )
            .map_err(|error| {
                EspaceSimulationError::Execution(super::EspaceExecutionError::Context {
                    source: error,
                })
            })?;
        let chain_id = u64::from(self.backend.chain_spec().espace_chain_id());
        let transaction =
            complete_transaction(transaction, self.backend.provider(), &context, chain_id).await?;
        let rules = self
            .backend
            .chain_spec()
            .espace_transaction_validation_rules(execution_block_number, execution_epoch_height);
        if let Some(rejection) = classify_transaction_rejection(&transaction, chain_id, rules) {
            return Ok(EspaceSimulation {
                context: context.public_context,
                transaction,
                execution: EspaceExecutionOutcome::NotExecuted(rejection),
                changes: super::EspaceChanges::Complete(super::EspaceChangeSet::default()),
            });
        }

        let state_source =
            ConfluxStateSource::prepare(context.state_anchor, self.backend.provider().clone())
                .await
                .map_err(|source| {
                    EspaceExecutionError::StateAccess(EspaceStateAccessError::Preparation {
                        source,
                    })
                })?;
        let backend = self.backend.clone();
        let limits = self.limits;
        let change_rules = Arc::clone(&self.change_rules);
        let blocking_runtime_handle = runtime_handle.clone();

        runtime_handle
            .spawn_blocking(move || {
                simulate_blocking(
                    backend,
                    blocking_runtime_handle,
                    context,
                    transaction,
                    Arc::new(state_source),
                    limits,
                    change_rules,
                )
            })
            .await
            .map_err(EspaceSimulationError::execution_task)?
    }
}

fn simulate_blocking<R>(
    backend: ConfluxSimulationBackend,
    runtime_handle: Handle,
    context: super::ResolvedEspaceContext,
    transaction: super::EspaceCompleteTransaction,
    state_source: Arc<ConfluxStateSource>,
    limits: EspaceSimulationLimits,
    change_rules: Arc<R>,
) -> Result<EspaceSimulation, EspaceSimulationError>
where
    R: super::EspaceChangeRules,
{
    let mut execution_state =
        build_conflux_state(Arc::clone(&state_source), runtime_handle.clone()).map_err(
            |source| {
                EspaceExecutionError::StateAccess(EspaceStateAccessError::Initialization { source })
            },
        )?;
    let machine = Arc::new(backend.chain_spec().build_machine());
    let execution_input = TransactionExecutionInput {
        block_context: context.execution_block_context,
        transaction: DryRunTransactionInput::Espace(build_executor_transaction(&transaction)?),
    };

    let observer = ExecutionTraceObserver::new(Space::Ethereum).with_log_checkpoints(
        change_rules.required_observations().into_log_checkpoints(),
        limits.max_occurrence_checkpoints,
    );
    let mut execution = ConfluxTransactionExecutor::new(&mut execution_state, &machine)
        .execute(execution_input, observer)
        .map_err(classify_executor_error)?;

    if matches!(
        &execution.outcome,
        ConfluxExecutionOutcome::NotExecutedDrop(_)
            | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_)
    ) {
        let outcome = convert_executor_outcome(
            execution.outcome,
            None,
            &transaction,
            None,
            backend.core_space_address_network(),
        )?;
        return Ok(EspaceSimulation {
            context: context.public_context,
            transaction,
            execution: outcome,
            changes: super::EspaceChanges::Complete(super::EspaceChangeSet::default()),
        });
    }

    let mut state = EspaceStateAccess::new(
        state_source,
        runtime_handle,
        execution_state,
        Arc::clone(&machine),
        &execution.prepared,
        transaction.common().from,
        limits,
    )
    .map_err(EspaceExecutionError::from)?;
    let record = EspaceExecutedTransaction::from_outcome(&mut execution.outcome, &mut state)?;

    let outcome = convert_executor_outcome(
        execution.outcome,
        Some(&record),
        &transaction,
        Some(state.finalized()),
        backend.core_space_address_network(),
    )?;
    let changes = super::EspaceChanges::from(change_rules.derive_changes(&record, &state));

    Ok(EspaceSimulation {
        context: context.public_context,
        transaction,
        execution: outcome,
        changes,
    })
}

fn classify_executor_error(
    error: crate::execution::TransactionExecutionError,
) -> super::EspaceExecutionError {
    use crate::execution::TransactionExecutionError;

    match error {
        TransactionExecutionError::BlockContext(source) => {
            super::EspaceExecutionError::Context { source }
        }
        TransactionExecutionError::StateAccess(source) => EspaceStateAccessError::Operation {
            operation: "execute eSpace transaction",
            source,
        }
        .into(),
        TransactionExecutionError::MissingExecutionTrace => EspaceResultIntegrationError::new(
            "executed transaction did not produce a committed execution trace",
        )
        .into(),
        TransactionExecutionError::GasValueOutOfRange { field, value } => {
            EspaceResultIntegrationError::new(format!(
                "executor returned {field} value {}, exceeding u64",
                crate::primitive::u256_from_cfx(value)
            ))
            .into()
        }
    }
}

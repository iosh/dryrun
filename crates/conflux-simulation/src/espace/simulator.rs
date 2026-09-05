use std::sync::Arc;

use cfx_types::Space;
use tokio::runtime::Handle;

use super::{
    EspaceChangesAnalysis, EspaceExecutedTransaction, EspaceExecutionError, EspaceExecutionOutcome,
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

#[derive(Clone)]
pub struct EspaceTransactionSimulator {
    backend: ConfluxSimulationBackend,
    limits: EspaceSimulationLimits,
}

impl EspaceTransactionSimulator {
    pub const fn new(backend: ConfluxSimulationBackend, limits: EspaceSimulationLimits) -> Self {
        Self { backend, limits }
    }

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
                changes: Vec::new(),
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
                )
            })
            .await
            .map_err(EspaceSimulationError::execution_task)?
    }
}

fn simulate_blocking(
    backend: ConfluxSimulationBackend,
    runtime_handle: Handle,
    context: super::ResolvedEspaceContext,
    transaction: super::EspaceCompleteTransaction,
    state_source: Arc<ConfluxStateSource>,
    limits: EspaceSimulationLimits,
) -> Result<EspaceSimulation, EspaceSimulationError> {
    // Keep S0 and the mutable execution state independent while sharing only
    // the anchored, request-local RPC source/cache.
    let initial_state = build_conflux_state(Arc::clone(&state_source), runtime_handle.clone())
        .map_err(|source| {
            EspaceExecutionError::StateAccess(EspaceStateAccessError::Initialization { source })
        })?;
    let mut execution_state = build_conflux_state(Arc::clone(&state_source), runtime_handle)
        .map_err(|source| {
            EspaceExecutionError::StateAccess(EspaceStateAccessError::Initialization { source })
        })?;
    let machine = Arc::new(backend.chain_spec().build_machine());
    let execution_input = TransactionExecutionInput {
        block_context: context.execution_block_context,
        transaction: DryRunTransactionInput::Espace(build_executor_transaction(&transaction)?),
    };

    let execution = ConfluxTransactionExecutor::new(&mut execution_state, &machine)
        .execute(
            execution_input,
            ExecutionTraceObserver::new(Space::Ethereum),
        )
        .map_err(classify_executor_error)?;

    let record = match &execution.outcome {
        ConfluxExecutionOutcome::Success(_) | ConfluxExecutionOutcome::Failed { .. } => {
            EspaceExecutedTransaction::from_outcome(&execution.outcome)?
        }
        ConfluxExecutionOutcome::NotExecutedDrop(_)
        | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_) => {
            let outcome = convert_executor_outcome(
                execution.outcome,
                None,
                &execution.prepared,
                &transaction,
                None,
                backend.core_space_address_network(),
            )?;
            return Ok(EspaceSimulation {
                context: context.public_context,
                transaction,
                execution: outcome,
                changes: Vec::new(),
            });
        }
    };

    let state = EspaceStateAccess::new(
        initial_state,
        execution_state,
        Arc::clone(&machine),
        execution.prepared.clone(),
        transaction.common().from,
        limits,
    );

    let analysis = EspaceChangesAnalysis::from_execution(
        &record,
        backend.chain_spec().espace_wrapped_native_token(),
        backend.chain_spec().espace_native_currency(),
    )?;
    let before_balances =
        analysis.read_native_balances(state.initial(), "read pre-execution native balances")?;
    let after_balances =
        analysis.read_native_balances(state.finalized(), "read post-execution native balances")?;

    let changes = analysis.finish(state.finalized(), &before_balances, &after_balances)?;

    let outcome = convert_executor_outcome(
        execution.outcome,
        Some(&record),
        &execution.prepared,
        &transaction,
        Some(state.finalized()),
        backend.core_space_address_network(),
    )?;

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

use cfx_types::Space;
use tokio::runtime::Handle;

use super::{
    EspaceChangesAnalysis, EspaceExecutionError, EspaceExecutionOutcome,
    EspaceResultIntegrationError, EspaceSimulation, EspaceSimulationError, EspaceSimulationRequest,
    EspaceStateAccessError, build_executor_transaction, classify_transaction_rejection,
    complete_transaction, convert_executor_outcome, resolve_espace_context,
    verify_observed_fee_settlement,
};
use crate::{
    ConfluxSimulationBackend,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecutor, DryRunTransactionInput,
        ObservationObserver, TransactionExecutionInput, build_conflux_state,
        next_execution_block_number, next_execution_epoch_height,
    },
    state::ConfluxStateSource,
};

#[derive(Clone)]
pub struct EspaceTransactionSimulator {
    backend: ConfluxSimulationBackend,
}

impl EspaceTransactionSimulator {
    pub const fn new(backend: ConfluxSimulationBackend) -> Self {
        Self { backend }
    }

    /// Simulates one eSpace transaction inside the caller's active Tokio runtime.
    pub async fn simulate(
        &self,
        request: EspaceSimulationRequest,
    ) -> Result<EspaceSimulation, EspaceSimulationError> {
        let EspaceSimulationRequest { block, transaction } = request;
        transaction.validate_requirements()?;
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
        let blocking_runtime_handle = runtime_handle.clone();

        runtime_handle
            .spawn_blocking(move || {
                simulate_blocking(
                    backend,
                    blocking_runtime_handle,
                    context,
                    transaction,
                    state_source,
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
    state_source: ConfluxStateSource,
) -> Result<EspaceSimulation, EspaceSimulationError> {
    let mut state = build_conflux_state(state_source, runtime_handle).map_err(|source| {
        EspaceExecutionError::StateAccess(EspaceStateAccessError::Initialization { source })
    })?;
    let machine = backend.chain_spec().build_machine();
    let execution_input = TransactionExecutionInput {
        block_context: context.execution_block_context,
        transaction: DryRunTransactionInput::Espace(build_executor_transaction(&transaction)?),
    };

    let state_before_execution = state.save();
    let execution = ConfluxTransactionExecutor::new(&mut state, &machine)
        .execute(execution_input, ObservationObserver::new(Space::Ethereum))
        .map_err(classify_executor_error)?;

    let execution_output = match &execution.outcome {
        ConfluxExecutionOutcome::Success(output) => Some((output, true)),
        ConfluxExecutionOutcome::Failed { details, .. } => Some((details, false)),
        ConfluxExecutionOutcome::NotExecutedDrop(_)
        | ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(_) => None,
    };

    let changes = if let Some((output, successful)) = execution_output {
        verify_observed_fee_settlement(output).map_err(EspaceExecutionError::from)?;
        let analysis = EspaceChangesAnalysis::from_execution(
            output,
            successful,
            backend.chain_spec().espace_wrapped_native_token(),
            backend.chain_spec().espace_native_currency(),
        )?;
        let state_after_execution = state.save();

        state.restore(state_before_execution);
        let before_balances =
            analysis.read_native_balances(&state, "read pre-execution native balances")?;
        state.restore(state_after_execution);
        let after_balances =
            analysis.read_native_balances(&state, "read post-execution native balances")?;

        analysis.finish(
            &mut state,
            &machine,
            &execution.prepared,
            &transaction,
            &before_balances,
            &after_balances,
        )?
    } else {
        Vec::new()
    };

    let outcome = convert_executor_outcome(
        execution.outcome,
        &execution.prepared,
        &transaction,
        &state,
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
        TransactionExecutionError::MissingObservations => {
            EspaceResultIntegrationError::invalid_executor_output(
                "executed transaction did not produce an observation journal",
            )
            .into()
        }
        TransactionExecutionError::GasValueOutOfRange { field, value } => {
            EspaceResultIntegrationError::invalid_executor_output(format!(
                "executor returned {field} value {value}, exceeding u64"
            ))
            .into()
        }
    }
}

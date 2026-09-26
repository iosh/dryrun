use std::sync::Arc;

use cfx_types::Space;
use simulation_core::simulation::Simulation;
use tokio::runtime::Handle;

use super::{
    EspaceExecutedTransaction, EspaceExecutionError, EspaceExecutionOutcome,
    EspaceResultIntegrationError, EspaceSimulation, EspaceSimulationError, EspaceSimulationLimits,
    EspaceSimulationRequest, EspaceStateAccess, EspaceStateAccessError, build_executor_transaction,
    complete_transaction, map_executor_outcome, prepare_espace_context,
};
use crate::{
    ConfluxSimulationBackend,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecutor, DryRunTransactionInput,
        ExecutionTraceObserver, TransactionExecutionInput, build_conflux_state,
    },
    state::ConfluxStateSource,
};

pub struct EspaceTransactionSimulator {
    backend: ConfluxSimulationBackend,
    limits: EspaceSimulationLimits,
    analyzers: Arc<super::EspaceAnalyzerRegistry>,
}

impl Clone for EspaceTransactionSimulator {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            limits: self.limits,
            analyzers: Arc::clone(&self.analyzers),
        }
    }
}

impl EspaceTransactionSimulator {
    pub fn new(backend: ConfluxSimulationBackend, limits: EspaceSimulationLimits) -> Self {
        let analyzers = super::analysis::default_registry(
            backend.chain_spec().espace_native_currency().clone(),
            super::changes::TokenAnalyzer::MAINNET_WCFX,
        );
        Self {
            backend,
            limits,
            analyzers: Arc::new(analyzers),
        }
    }
}

impl EspaceTransactionSimulator {
    pub fn with_analyzers(mut self, analyzers: super::EspaceAnalyzerRegistry) -> Self {
        self.analyzers = Arc::new(analyzers);
        self
    }
    pub fn with_analyzer(
        mut self,
        analyzer: impl simulation_core::analysis::Analyzer<super::EspaceAnalysisDomain>,
    ) -> Result<Self, simulation_core::analysis::RegistryError> {
        self.analyzers = Arc::new(self.analyzers.with_analyzer(analyzer)?);
        Ok(self)
    }

    /// Simulates one eSpace transaction inside the caller's active Tokio runtime.
    /// Uses a fixed epoch and checks its pivot before execution and result delivery.
    /// Separate state RPCs do not provide an atomic snapshot during a reorganization.
    pub async fn simulate(
        &self,
        request: EspaceSimulationRequest,
    ) -> Result<EspaceSimulation, EspaceSimulationError> {
        let simulation =
            simulation_core::simulation::simulate(EspaceBackend(self.clone()), request).await?;
        let context = match &simulation {
            Simulation::Rejected(result) => result.context(),
            Simulation::Executed(result) => Some(result.context()),
        };
        if let Some(context) = context {
            self.backend
                .provider()
                .validate_state_anchor(context.state_anchor())
                .await
                .map_err(super::EspaceContextError::from)?;
        }
        Ok(simulation)
    }
}

struct EspaceBackend(EspaceTransactionSimulator);
struct PreparedEspaceExecution {
    context: crate::context::ExecutionBlockContext,
    state: Arc<ConfluxStateSource>,
}

struct EspaceExecutionEvidence {
    outcome: EspaceExecutionOutcome,
    record: EspaceExecutedTransaction,
}

impl simulation_core::simulation::SimulationBackend for EspaceBackend {
    type Request = EspaceSimulationRequest;
    type Context = super::EspaceBlockContext;
    type Transaction = super::EspaceTypedTransaction;
    type TransactionRequest = super::EspaceTransactionRequest;
    type Rejection = super::EspaceTransactionRejection;
    type Prepared = PreparedEspaceExecution;
    type Evidence = EspaceExecutionEvidence;
    type Outcome = EspaceExecutionOutcome;
    type ChangeSet = super::EspaceChangeSet;
    type AnalysisError = super::EspaceAnalysisError;
    type Error = EspaceSimulationError;

    async fn prepare(
        &self,
        request: Self::Request,
    ) -> Result<simulation_core::simulation::PreparationFor<Self>, Self::Error> {
        use simulation_core::{completion::Completion, simulation::Preparation};
        let EspaceSimulationRequest { block, transaction } = request;
        let backend = &self.0.backend;
        let context = prepare_espace_context(
            backend.provider(),
            block,
            backend.chain_spec().common_params(),
        )
        .await?;
        let execution_block_number = context.execution_block_context.number;
        let execution_epoch_height = context.execution_block_context.epoch_height;
        let chain_id = u64::from(backend.chain_spec().espace_chain_id());
        let rules = backend
            .chain_spec()
            .espace_transaction_validation_rules(execution_block_number, execution_epoch_height);
        let transaction =
            match complete_transaction(transaction, backend.provider(), &context, chain_id, rules)
                .await?
            {
                Completion::Ready(transaction) => transaction,
                Completion::Rejected {
                    transaction,
                    rejection,
                } => {
                    return Ok(Preparation::Rejected {
                        context: Some(context.public_context),
                        transaction,
                        rejection,
                    });
                }
            };
        let state = ConfluxStateSource::prepare(context.state_anchor, backend.provider().clone())
            .await
            .map_err(|source| {
                EspaceExecutionError::StateAccess(EspaceStateAccessError::Preparation { source })
            })?;
        backend
            .provider()
            .validate_state_anchor(context.state_anchor)
            .await
            .map_err(super::EspaceContextError::from)?;
        Ok(Preparation::Ready {
            context: context.public_context,
            transaction,
            execution: PreparedEspaceExecution {
                context: context.execution_block_context,
                state: Arc::new(state),
            },
        })
    }

    fn execute(
        &self,
        transaction: &Self::Transaction,
        prepared: Self::Prepared,
        runtime: Handle,
    ) -> Result<simulation_core::simulation::Execution<Self::Evidence, Self::Rejection>, Self::Error>
    {
        use simulation_core::simulation::Execution;
        let backend = &self.0.backend;
        let mut execution_state = build_conflux_state(Arc::clone(&prepared.state), runtime.clone())
            .map_err(|source| {
                EspaceExecutionError::StateAccess(EspaceStateAccessError::Initialization { source })
            })?;
        let machine = Arc::new(backend.chain_spec().build_machine());
        let input = TransactionExecutionInput {
            block_context: prepared.context,
            transaction: DryRunTransactionInput::Espace(build_executor_transaction(transaction)?),
        };
        let observer = ExecutionTraceObserver::new(Space::Ethereum).with_checkpoint_filters(
            crate::execution::filters_for_space(
                self.0.analyzers.checkpoint_filters(),
                Space::Ethereum,
            ),
            self.0.limits,
        );
        let mut execution = ConfluxTransactionExecutor::new(&mut execution_state, &machine)
            .execute(input, observer)
            .map_err(map_execution_error)?;
        match execution.outcome {
            ConfluxExecutionOutcome::NotExecutedDrop(error) => {
                return Ok(Execution::Rejected(super::outcome_mapping::map_drop_error(
                    error,
                )?));
            }
            ConfluxExecutionOutcome::NotExecutedToReconsiderPacking(error) => {
                return Ok(Execution::Rejected(
                    super::outcome_mapping::map_reconsider_packing_error(error)?,
                ));
            }
            _ => {}
        }
        let state = EspaceStateAccess::new(
            prepared.state,
            runtime,
            execution_state,
            machine,
            &execution.prepared,
            transaction.common().from,
            self.0.limits,
        )
        .map_err(EspaceExecutionError::from)?;
        let record = EspaceExecutedTransaction::from_outcome(&mut execution.outcome, state)?;
        let outcome = map_executor_outcome(
            execution.outcome,
            &record,
            transaction,
            record.state().finalized(),
            backend.core_space_address_network(),
        )?;
        record.state().start_analysis();
        Ok(Execution::Executed(EspaceExecutionEvidence {
            outcome,
            record,
        }))
    }

    fn is_success(&self, evidence: &Self::Evidence) -> bool {
        evidence.record.is_success()
    }

    fn analyze(
        &self,
        view: simulation_core::simulation::AnalysisView<
            '_,
            Self::Context,
            Self::Transaction,
            Self::Evidence,
        >,
    ) -> Result<Self::ChangeSet, Self::AnalysisError> {
        let evidence = view.execution();
        evidence.record.check_observation_limit()?;
        let view = super::EspaceAnalysisView {
            context: view.context(),
            transaction: view.transaction(),
            execution: &evidence.record,
            core_chain_id: self.0.backend.chain_spec().core_space_chain_id(),
        };
        let mut changes = self.0.analyzers.analyze(view)?;
        changes.load_metadata(evidence.record.state().finalized())?;
        changes.set_space(simulation_core::analysis::ExecutionSpace::Espace);
        Ok(changes)
    }

    fn into_outcome(&self, evidence: Self::Evidence) -> Self::Outcome {
        evidence.outcome
    }
}

fn map_execution_error(
    error: crate::execution::TransactionExecutionError,
) -> super::EspaceExecutionError {
    use crate::execution::TransactionExecutionError;

    match error {
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

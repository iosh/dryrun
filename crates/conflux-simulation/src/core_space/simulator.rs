use simulation_core::simulation::Simulation;
use std::sync::Arc;

use tokio::runtime::Handle;

use crate::{ConfluxSimulationBackend, state::ConfluxStateSource};

use super::{
    CoreSpaceExecutionError, CoreSpaceExecutionOutcome, CoreSpaceSimulation,
    CoreSpaceSimulationError, CoreSpaceSimulationRequest, CoreSpaceStateAccessError,
    CoreSpaceTransactionRejection, CoreSpaceTypedTransaction, check_storage_sponsorship,
    complete_transaction, prepare_core_space_context, session::CoreSpaceExecutionSession,
};

pub struct CoreSpaceTransactionSimulator<R = super::DefaultCoreSpaceChangeRules> {
    backend: ConfluxSimulationBackend,
    limits: super::CoreSpaceSimulationLimits,
    change_rules: Arc<R>,
}

impl<R> Clone for CoreSpaceTransactionSimulator<R> {
    fn clone(&self) -> Self {
        Self {
            backend: self.backend.clone(),
            limits: self.limits,
            change_rules: Arc::clone(&self.change_rules),
        }
    }
}

impl CoreSpaceTransactionSimulator<super::DefaultCoreSpaceChangeRules> {
    pub fn new(backend: ConfluxSimulationBackend) -> Self {
        Self::with_limits(backend, super::CoreSpaceSimulationLimits::default())
    }

    pub fn with_limits(
        backend: ConfluxSimulationBackend,
        limits: super::CoreSpaceSimulationLimits,
    ) -> Self {
        let change_rules = super::DefaultCoreSpaceChangeRules::new_with_espace(
            backend.chain_spec().core_space_native_currency().clone(),
            backend.chain_spec().espace_native_currency().clone(),
            crate::espace::DefaultEspaceChangeRules::MAINNET_WCFX,
        );
        Self {
            backend,
            limits,
            change_rules: Arc::new(change_rules),
        }
    }
}

impl<R> CoreSpaceTransactionSimulator<R> {
    pub fn with_change_rules<N>(self, change_rules: N) -> CoreSpaceTransactionSimulator<N>
    where
        N: super::CoreSpaceChangeRules,
    {
        CoreSpaceTransactionSimulator {
            backend: self.backend,
            limits: self.limits,
            change_rules: Arc::new(change_rules),
        }
    }

    pub fn with_additional_change_rules<N>(
        self,
        change_rules: N,
    ) -> CoreSpaceTransactionSimulator<super::CombinedCoreSpaceChangeRules<R, N>>
    where
        R: super::CoreSpaceChangeRules,
        N: super::CoreSpaceChangeRules,
    {
        CoreSpaceTransactionSimulator {
            backend: self.backend,
            limits: self.limits,
            change_rules: Arc::new(super::CombinedCoreSpaceChangeRules::from_shared(
                self.change_rules,
                change_rules,
            )),
        }
    }
}

impl<R: super::CoreSpaceChangeRules> CoreSpaceTransactionSimulator<R> {
    /// Simulates one Core Space transaction inside the caller's active Tokio runtime.
    /// Uses a fixed epoch and checks its pivot before execution and result delivery.
    /// Separate state RPCs do not provide an atomic snapshot during a reorganization.
    pub async fn simulate(
        &self,
        request: CoreSpaceSimulationRequest,
    ) -> Result<CoreSpaceSimulation, CoreSpaceSimulationError> {
        let simulation =
            simulation_core::simulation::simulate(CoreSpaceBackend(self.clone()), request).await?;
        let context = match &simulation {
            Simulation::Rejected(result) => result.context(),
            Simulation::Executed(result) => Some(result.context()),
        };
        if let Some(context) = context {
            self.backend
                .provider()
                .validate_state_anchor(context.state_anchor())
                .await
                .map_err(super::CoreSpaceContextError::from)?;
        }
        Ok(simulation)
    }
}

struct CoreSpaceBackend<R>(CoreSpaceTransactionSimulator<R>);
struct PreparedCoreSpaceExecution {
    context: crate::context::ExecutionBlockContext,
    storage_sponsorship: Option<super::StorageSponsorship>,
    state: ConfluxStateSource,
}

impl<R: super::CoreSpaceChangeRules> simulation_core::simulation::SimulationBackend
    for CoreSpaceBackend<R>
{
    type Request = CoreSpaceSimulationRequest;
    type Context = super::CoreSpaceBlockContext;
    type Transaction = CoreSpaceTypedTransaction;
    type TransactionRequest = super::CoreSpaceTransactionRequest;
    type Rejection = CoreSpaceTransactionRejection;
    type Prepared = PreparedCoreSpaceExecution;
    type Evidence = super::session::CoreSpaceExecutionEvidence;
    type Outcome = CoreSpaceExecutionOutcome;
    type ChangeSet = super::CoreSpaceChangeSet;
    type AnalysisError = super::CoreSpaceAnalysisError;
    type Error = CoreSpaceSimulationError;

    async fn prepare(
        &self,
        request: Self::Request,
    ) -> Result<simulation_core::simulation::PreparationFor<Self>, Self::Error> {
        use simulation_core::{completion::Completion, simulation::Preparation};
        let CoreSpaceSimulationRequest { block, transaction } = request;
        let backend = &self.0.backend;
        super::transaction::validate_address_networks(
            &transaction,
            backend.core_space_address_network(),
        )?;
        let context = prepare_core_space_context(
            backend.provider(),
            block,
            backend.chain_spec().common_params(),
        )
        .await?;
        let execution_block_number = context.execution_block_context.number;
        let execution_epoch_height = context.execution_block_context.epoch_height;
        let rules = backend
            .chain_spec()
            .core_space_transaction_validation_rules(
                execution_block_number,
                execution_epoch_height,
            );
        let transaction = match complete_transaction(
            transaction,
            backend.provider(),
            &context,
            backend.chain_spec().core_space_chain_id(),
            rules,
        )
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
        let spec = backend
            .chain_spec()
            .common_params()
            .spec(execution_block_number, execution_epoch_height);
        let storage_sponsorship = if spec.cip78a || spec.cip78b {
            Some(
                check_storage_sponsorship(backend.provider(), context.state_anchor, &transaction)
                    .await?,
            )
        } else {
            None
        };
        let state = ConfluxStateSource::prepare(context.state_anchor, backend.provider().clone())
            .await
            .map_err(|source| {
                CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Preparation {
                    source,
                })
            })?;
        backend
            .provider()
            .validate_state_anchor(context.state_anchor)
            .await
            .map_err(super::CoreSpaceContextError::from)?;
        Ok(Preparation::Ready {
            context: context.public_context,
            transaction,
            execution: PreparedCoreSpaceExecution {
                context: context.execution_block_context,
                storage_sponsorship,
                state,
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
        let session = CoreSpaceExecutionSession::new(&self.0.backend, prepared.state, runtime)?;
        session.execute(
            transaction,
            prepared.context,
            prepared.storage_sponsorship,
            &self.0.change_rules.checkpoint_filters(),
            self.0.limits,
        )
    }

    fn is_success(&self, evidence: &Self::Evidence) -> bool {
        evidence.record.status() == super::CoreSpaceExecutionStatus::Success
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
        evidence.record.state().start_analysis();
        let view = super::CoreSpaceAnalysisView {
            context: view.context(),
            transaction: view.transaction(),
            execution: &evidence.record,
        };
        super::changes::check_contract_support(view.execution(), view.state())?;
        self.0.change_rules.derive_changes(view)
    }

    fn into_outcome(&self, evidence: Self::Evidence) -> Self::Outcome {
        evidence.outcome
    }
}

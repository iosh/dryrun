use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use conflux_provider::Network;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend,
    espace::EspaceNativeCurrency,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecutor, DryRunTransactionInput,
        ExecutionBlockContext, ExecutionTraceObserver, TransactionExecutionInput,
        build_conflux_state,
    },
    state::ConfluxStateSource,
};

use super::{
    CoreSpaceChange, CoreSpaceCompleteTransaction, CoreSpaceExecutionError,
    CoreSpaceExecutionOutcome, CoreSpaceNativeCurrency, CoreSpaceSimulationError,
    CoreSpaceStateAccessError, ResolvedStorageSponsorship,
    analysis::{CoreSpaceAnalysisData, CoreSpaceChangeAnalysis},
    changes::StatePhase,
    convert_executor_outcome,
    transaction::build_core_space_transaction_input,
};

pub(super) struct CoreSpaceExecutionSession {
    state: State,
    machine: Machine,
    chain_id: u32,
    network: Network,
    currency: CoreSpaceNativeCurrency,
    espace_currency: EspaceNativeCurrency,
    espace_wrapped_native_token: alloy_primitives::Address,
    analysis_data: CoreSpaceAnalysisData,
}

pub(super) struct CoreSpaceExecutionSessionResult {
    pub(super) outcome: CoreSpaceExecutionOutcome,
    pub(super) changes: Vec<CoreSpaceChange>,
}

impl CoreSpaceExecutionSession {
    pub(super) fn new(
        backend: &ConfluxSimulationBackend,
        state_source: ConfluxStateSource,
        runtime_handle: Handle,
    ) -> Result<Self, CoreSpaceExecutionError> {
        let analysis_data = CoreSpaceAnalysisData::from_state_source(&state_source);
        let state = build_conflux_state(state_source, runtime_handle).map_err(|source| {
            CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Initialization {
                source,
            })
        })?;

        Ok(Self {
            state,
            machine: backend.chain_spec().build_machine(),
            chain_id: backend.chain_spec().core_space_chain_id(),
            network: backend.core_space_address_network(),
            currency: backend.chain_spec().core_space_native_currency().clone(),
            espace_currency: backend.chain_spec().espace_native_currency().clone(),
            espace_wrapped_native_token: backend.chain_spec().espace_wrapped_native_token(),
            analysis_data,
        })
    }

    pub(super) fn execute(
        mut self,
        transaction: &CoreSpaceCompleteTransaction,
        block_context: ExecutionBlockContext,
        storage_sponsorship: ResolvedStorageSponsorship,
    ) -> Result<CoreSpaceExecutionSessionResult, CoreSpaceSimulationError> {
        let execution_input = TransactionExecutionInput {
            block_context,
            transaction: DryRunTransactionInput::CoreSpace(build_core_space_transaction_input(
                transaction,
                self.chain_id,
            )),
        };
        let state_before_execution = self.state.save();
        let execution = ConfluxTransactionExecutor::new(&mut self.state, &self.machine)
            .execute(execution_input, ExecutionTraceObserver::new(Space::Native))
            .map_err(classify_transaction_execution_error)?;

        let changes = if matches!(&execution.outcome, ConfluxExecutionOutcome::Success(_)) {
            let mut analysis = CoreSpaceChangeAnalysis::from_execution(
                &execution,
                &self.machine,
                self.analysis_data,
                self.network,
                &self.currency,
                &self.espace_currency,
                self.espace_wrapped_native_token,
            )?;
            let state_after_execution = self.state.save();

            self.state.restore(state_before_execution);
            let before = analysis.read_state(
                &mut self.state,
                &self.machine,
                &execution.prepared,
                StatePhase::Before,
            )?;
            self.state.restore(state_after_execution);

            let state_before_after_reads = self.state.save();
            let after = analysis.read_state(
                &mut self.state,
                &self.machine,
                &execution.prepared,
                StatePhase::After,
            )?;
            self.state.restore(state_before_after_reads);

            analysis.analyze(
                &mut self.state,
                &self.machine,
                &execution.prepared,
                before,
                after,
            )?
        } else {
            Vec::new()
        };

        let outcome = convert_executor_outcome(
            execution.outcome,
            &execution.prepared,
            transaction,
            &self.state,
            storage_sponsorship,
        )?;

        Ok(CoreSpaceExecutionSessionResult { outcome, changes })
    }
}

fn classify_transaction_execution_error(
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

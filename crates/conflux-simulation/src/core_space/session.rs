use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::legacy::StatePhase;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationBackend, ConfluxSimulationError,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecutor, DryRunTransactionInput,
        ExecutionBlockContext, ExecutionTraceObserver, TransactionExecutionInput,
        build_conflux_state,
    },
    state::{AnchoredVoteLists, ConfluxStateSource, MaskedSponsorWhitelistEntries},
};

use super::{
    CoreSpaceChange, CoreSpaceCompleteTransaction, CoreSpaceExecutionError,
    CoreSpaceExecutionOutcome, CoreSpaceStateAccessError, ResolvedStorageSponsorship,
    analysis::CoreSpaceChangeAnalysis, convert_executor_outcome,
    transaction::build_core_space_transaction_input,
};

pub(super) struct CoreSpaceExecutionSession {
    state: State,
    machine: Machine,
    chain_id: u32,
    masked_sponsor_whitelist_entries: MaskedSponsorWhitelistEntries,
    anchored_vote_lists: AnchoredVoteLists,
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
    ) -> Result<Self, ConfluxSimulationError> {
        let masked_sponsor_whitelist_entries = state_source.masked_sponsor_whitelist_entries();
        let anchored_vote_lists = state_source.anchored_vote_lists();
        let state = build_conflux_state(state_source, runtime_handle).map_err(|source| {
            CoreSpaceExecutionError::StateAccess(CoreSpaceStateAccessError::Initialization {
                source,
            })
        })?;

        Ok(Self {
            state,
            machine: backend.chain_spec().build_machine(),
            chain_id: backend.chain_spec().core_space_chain_id(),
            masked_sponsor_whitelist_entries,
            anchored_vote_lists,
        })
    }

    pub(super) fn execute(
        mut self,
        transaction: &CoreSpaceCompleteTransaction,
        block_context: ExecutionBlockContext,
        storage_sponsorship: ResolvedStorageSponsorship,
    ) -> Result<CoreSpaceExecutionSessionResult, ConfluxSimulationError> {
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
            .map_err(CoreSpaceExecutionError::from)?;

        let changes = if matches!(&execution.outcome, ConfluxExecutionOutcome::Success(_)) {
            let mut analysis = CoreSpaceChangeAnalysis::from_execution(
                &execution,
                &self.machine,
                &self.masked_sponsor_whitelist_entries,
                &self.anchored_vote_lists,
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

use cfx_executor::{machine::Machine, state::State};
use cfx_types::U256;
use conflux_provider::Network;

use crate::{
    core_space::CoreSpaceChangesError,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecution, PreparedTransactionExecution,
    },
    state::{ConfluxStateSource, MaskedWhitelistKeys, RecordedDepositLists, RecordedVoteLists},
};

use super::changes::{
    ActiveContracts, CfxAnalysisInput, CfxStateValues, CommittedCalls, CoreSpaceChange,
    CoreSpaceNativeCurrency, PoSAnalysisInput, PoSStateReader, PoSStateValues,
    PositionedCoreSpaceChange, StatePhase, analyze_balance_changes, collect_calls,
    collect_standard_changes, finish_core_space_changes, load_standard_metadata,
    verify_pos_staking_changes, verify_vote_lock_changes,
};

struct CoreSpaceAnalysisInput {
    cfx: CfxAnalysisInput,
    calls: CommittedCalls,
    pos: PoSAnalysisInput,
    standard_changes: Vec<PositionedCoreSpaceChange>,
}

pub(super) struct CoreSpaceAnalysisData {
    masked_whitelist_keys: MaskedWhitelistKeys,
    deposit_lists: RecordedDepositLists,
    vote_lists: RecordedVoteLists,
    accumulated_interest_rate: U256,
}

impl CoreSpaceAnalysisData {
    pub(super) fn from_state_source(state_source: &ConfluxStateSource) -> Self {
        Self {
            masked_whitelist_keys: state_source.masked_whitelist_keys(),
            deposit_lists: state_source.deposit_lists(),
            vote_lists: state_source.vote_lists(),
            accumulated_interest_rate: state_source.accumulated_interest_rate(),
        }
    }
}

impl CoreSpaceAnalysisInput {
    fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        masked_whitelist_keys: &MaskedWhitelistKeys,
    ) -> Result<Self, CoreSpaceChangesError> {
        let ConfluxExecutionOutcome::Success(details) = &execution.outcome else {
            return Err(CoreSpaceChangesError::internal_invariant(
                "successful execution is required for Core Space analysis",
            ));
        };

        let active_contracts =
            ActiveContracts::from_machine_and_spec(machine, &execution.prepared.spec);
        let calls = collect_calls(&details.trace, active_contracts)?;
        let cfx = CfxAnalysisInput::from_execution(
            execution,
            machine,
            masked_whitelist_keys,
            calls.staking_calls(),
        )?;
        let pos = PoSAnalysisInput::from_calls_and_logs(
            calls.pos_calls(),
            &details.logs,
            active_contracts.pos_register_is_active(),
        )?;
        let standard_changes = collect_standard_changes(details)?;

        Ok(Self {
            cfx,
            calls,
            pos,
            standard_changes,
        })
    }
}

#[derive(Debug)]
pub(super) struct CoreSpaceStateValues {
    cfx: CfxStateValues,
    pos: Option<PoSStateValues>,
}

#[derive(Default)]
struct CoreSpaceStateReader {
    pos_state_reader: PoSStateReader,
}

impl CoreSpaceStateReader {
    fn read(
        &mut self,
        state: &mut State,
        _machine: &Machine,
        _prepared_execution: &PreparedTransactionExecution,
        analysis_input: &CoreSpaceAnalysisInput,
        phase: StatePhase,
    ) -> Result<CoreSpaceStateValues, CoreSpaceChangesError> {
        let cfx = analysis_input.cfx.read_state(state, phase)?;
        let pos = self.pos_state_reader.read(
            state,
            analysis_input.calls.pos_calls(),
            analysis_input.pos.state_requirements(),
            phase,
        )?;
        Ok(CoreSpaceStateValues { cfx, pos })
    }
}

pub(super) struct CoreSpaceChangeAnalysis {
    input: CoreSpaceAnalysisInput,
    state_reader: CoreSpaceStateReader,
    data: CoreSpaceAnalysisData,
    network: Network,
    currency: CoreSpaceNativeCurrency,
}

impl CoreSpaceChangeAnalysis {
    pub(super) fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        data: CoreSpaceAnalysisData,
        network: Network,
        currency: &CoreSpaceNativeCurrency,
    ) -> Result<Self, CoreSpaceChangesError> {
        Ok(Self {
            input: CoreSpaceAnalysisInput::from_execution(
                execution,
                machine,
                &data.masked_whitelist_keys,
            )?,
            state_reader: CoreSpaceStateReader::default(),
            data,
            network,
            currency: currency.clone(),
        })
    }

    pub(super) fn read_state(
        &mut self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        phase: StatePhase,
    ) -> Result<CoreSpaceStateValues, CoreSpaceChangesError> {
        self.state_reader
            .read(state, machine, prepared_execution, &self.input, phase)
    }

    pub(super) fn analyze(
        self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        before: CoreSpaceStateValues,
        after: CoreSpaceStateValues,
    ) -> Result<Vec<CoreSpaceChange>, CoreSpaceChangesError> {
        let Self {
            input: analysis_input,
            data,
            network,
            currency,
            ..
        } = self;
        let CoreSpaceAnalysisData {
            deposit_lists,
            vote_lists,
            accumulated_interest_rate,
            ..
        } = data;
        let CoreSpaceStateValues {
            cfx: before_cfx_state,
            pos: before_pos_state,
        } = before;
        let CoreSpaceStateValues {
            cfx: after_cfx_state,
            pos: after_pos_state,
        } = after;
        let mut positioned_core_changes = analysis_input
            .cfx
            .verify(&before_cfx_state, &after_cfx_state)?;
        positioned_core_changes.extend(analyze_balance_changes(
            state,
            analysis_input.calls.staking_calls(),
            &deposit_lists,
            accumulated_interest_rate,
            prepared_execution.env.number,
            prepared_execution.spec.cip97,
            &before_cfx_state,
        )?);
        positioned_core_changes.extend(verify_vote_lock_changes(
            state,
            analysis_input.calls.staking_calls(),
            &vote_lists,
            prepared_execution.env.number,
            &before_cfx_state,
        )?);

        match (before_pos_state, after_pos_state) {
            (Some(before), Some(after)) => {
                positioned_core_changes.extend(verify_pos_staking_changes(
                    &analysis_input.pos,
                    &before,
                    &after,
                    analysis_input.cfx.staking_balance_effects(),
                )?);
            }
            (None, None) if analysis_input.pos.events().is_empty() => {}
            (None, None) => {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space PoS final logs had no matching committed call",
                ));
            }
            _ => {
                return Err(CoreSpaceChangesError::inconsistent_execution(
                    "Core Space PoS before and after states were inconsistent",
                ));
            }
        }

        positioned_core_changes.extend(analysis_input.standard_changes);

        if positioned_core_changes.is_empty() {
            return Ok(Vec::new());
        }

        let metadata =
            load_standard_metadata(state, machine, prepared_execution, &positioned_core_changes)?;
        finish_core_space_changes(positioned_core_changes, &metadata, network, &currency)
    }
}

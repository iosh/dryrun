use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::legacy::{
    StandardCandidate, StandardStateValues, StatePhase, StateRequirements, metadata_requests,
    state_requirements, verify,
};

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecution, PreparedTransactionExecution,
    },
    standards::{collect_standard_candidates, load_change_metadata, read_standard_state_values},
    state::{AnchoredVoteLists, MaskedSponsorWhitelistEntries, StatePhaseValues},
};

use super::changes::{
    CfxAnalysisInput, CfxStateValues, CommittedStakingCalls, CoreSpaceChange, PoSAnalysisInput,
    PoSStateReader, PoSStateValues, PositionedCoreSpaceChange, StakingContractActivation,
    collect_committed_staking_calls, order_and_enrich_core_space_changes,
    verify_pos_staking_changes, verify_vote_lock_changes,
};

struct CoreSpaceAnalysisInput {
    cfx: CfxAnalysisInput,
    committed_staking_calls: CommittedStakingCalls,
    pos: PoSAnalysisInput,
    standard_candidates: Vec<StandardCandidate>,
    standard_state_requirements: StateRequirements,
}

impl CoreSpaceAnalysisInput {
    fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        masked_sponsor_whitelist_entries: &MaskedSponsorWhitelistEntries,
    ) -> Result<Self, ConfluxSimulationError> {
        let ConfluxExecutionOutcome::Success(details) = &execution.outcome else {
            return Err(ConfluxSimulationError::ExecutionInternal {
                message: "successful execution is required for Core Space analysis".into(),
            });
        };

        let cfx =
            CfxAnalysisInput::from_execution(execution, machine, masked_sponsor_whitelist_entries)?;

        let staking_contract_activation =
            StakingContractActivation::from_machine_and_spec(machine, &execution.prepared.spec);
        let committed_staking_calls =
            collect_committed_staking_calls(&details.observations, staking_contract_activation)?;
        let pos = PoSAnalysisInput::from_calls_and_logs(
            committed_staking_calls.pos_calls(),
            &details.logs,
            staking_contract_activation.pos_register_is_active(),
        )?;
        let standard_candidates =
            collect_standard_candidates(&details.observations, Space::Native)?;
        let standard_state_requirements = state_requirements(&standard_candidates);

        Ok(Self {
            cfx,
            committed_staking_calls,
            pos,
            standard_candidates,
            standard_state_requirements,
        })
    }
}

#[derive(Debug)]
pub(super) struct CoreSpaceStateValues {
    cfx: CfxStateValues,
    pos: Option<PoSStateValues>,
    standards: StandardStateValues,
}

#[derive(Default)]
struct CoreSpaceStateReader {
    pos_state_reader: PoSStateReader,
}

impl CoreSpaceStateReader {
    fn read(
        &mut self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        analysis_input: &CoreSpaceAnalysisInput,
        phase: StatePhase,
    ) -> Result<CoreSpaceStateValues, ConfluxSimulationError> {
        let cfx = analysis_input.cfx.read_state(state, phase)?;
        let pos = self.pos_state_reader.read(
            state,
            analysis_input.committed_staking_calls.pos_calls(),
            analysis_input.pos.state_requirements(),
            phase,
        )?;
        let standards = read_standard_state_values(
            state,
            machine,
            prepared_execution,
            phase,
            &analysis_input.standard_state_requirements,
        )?;

        Ok(CoreSpaceStateValues {
            cfx,
            pos,
            standards,
        })
    }
}

pub(super) struct CoreSpaceChangeAnalysis {
    input: CoreSpaceAnalysisInput,
    state_reader: CoreSpaceStateReader,
    anchored_vote_lists: AnchoredVoteLists,
}

impl CoreSpaceChangeAnalysis {
    pub(super) fn from_execution(
        execution: &ConfluxTransactionExecution,
        machine: &Machine,
        masked_sponsor_whitelist_entries: &MaskedSponsorWhitelistEntries,
        anchored_vote_lists: &AnchoredVoteLists,
    ) -> Result<Self, ConfluxSimulationError> {
        Ok(Self {
            input: CoreSpaceAnalysisInput::from_execution(
                execution,
                machine,
                masked_sponsor_whitelist_entries,
            )?,
            state_reader: CoreSpaceStateReader::default(),
            anchored_vote_lists: anchored_vote_lists.clone(),
        })
    }

    pub(super) fn read_state(
        &mut self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        phase: StatePhase,
    ) -> Result<CoreSpaceStateValues, ConfluxSimulationError> {
        self.state_reader
            .read(state, machine, prepared_execution, &self.input, phase)
    }

    pub(super) fn analyze(
        self,
        state: &mut State,
        machine: &Machine,
        prepared_execution: &PreparedTransactionExecution,
        phase_values: StatePhaseValues<CoreSpaceStateValues>,
    ) -> Result<Vec<CoreSpaceChange>, ConfluxSimulationError> {
        let Self {
            input: analysis_input,
            anchored_vote_lists,
            ..
        } = self;
        let StatePhaseValues { before, after } = phase_values;
        let CoreSpaceStateValues {
            cfx: before_cfx_state,
            pos: before_pos_state,
            standards: before_standard_state,
        } = before;
        let CoreSpaceStateValues {
            cfx: after_cfx_state,
            pos: after_pos_state,
            standards: after_standard_state,
        } = after;

        let positioned_standard_changes = verify(
            &analysis_input.standard_candidates,
            &before_standard_state,
            &after_standard_state,
        )?;
        let metadata_requests = metadata_requests(&positioned_standard_changes);
        let mut positioned_core_changes = analysis_input
            .cfx
            .verify(&before_cfx_state, &after_cfx_state)?;
        positioned_core_changes.extend(verify_vote_lock_changes(
            state,
            analysis_input.committed_staking_calls.vote_lock_calls(),
            &anchored_vote_lists,
            prepared_execution.env.number,
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
                return Err(ConfluxSimulationError::analysis_failed(
                    "Core Space PoS final logs had no matching committed call",
                ));
            }
            _ => {
                return Err(ConfluxSimulationError::ExecutionInternal {
                    message: "Core Space PoS before and after states were inconsistent".into(),
                });
            }
        }

        positioned_core_changes.extend(
            positioned_standard_changes
                .into_iter()
                .map(PositionedCoreSpaceChange::from),
        );

        if positioned_core_changes.is_empty() {
            return Ok(Vec::new());
        }

        let metadata =
            load_change_metadata(state, machine, prepared_execution, &metadata_requests)?;
        Ok(order_and_enrich_core_space_changes(
            positioned_core_changes,
            &metadata,
        ))
    }
}

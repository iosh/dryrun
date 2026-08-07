use cfx_executor::{machine::Machine, state::State};
use cfx_types::Space;
use contract_standards::{
    MetadataRequests, StandardCandidate, StandardStateValues, StatePhase, StateRequirements,
    state_requirements, verify,
};
use simulation_changes::{
    Change, PositionedChange, into_enriched_changes, sort_changes_by_position,
};

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxExecutionOutcome, ConfluxTransactionExecution, PreparedTransactionExecution,
    },
    standards::{collect_standard_candidates, load_change_metadata, read_standard_state_values},
    state::StatePhaseValues,
};

use super::changes::{EspaceNativeAnalysis, NativeBalances};

pub(crate) struct EspaceAnalysisInput {
    native: EspaceNativeAnalysis,
    standard_candidates: Vec<StandardCandidate>,
    standard_state_requirements: StateRequirements,
}

impl EspaceAnalysisInput {
    pub(crate) fn from_execution(
        execution: &ConfluxTransactionExecution,
    ) -> Result<Self, ConfluxSimulationError> {
        let ConfluxExecutionOutcome::Success(details) = &execution.outcome else {
            return Err(ConfluxSimulationError::ExecutionInternal {
                message: "successful execution is required for eSpace analysis".into(),
            });
        };

        let standard_candidates =
            collect_standard_candidates(&details.observations, Space::Ethereum)?;
        let standard_state_requirements = state_requirements(&standard_candidates);

        Ok(Self {
            native: EspaceNativeAnalysis::from_execution(details)?,
            standard_candidates,
            standard_state_requirements,
        })
    }
}

pub(crate) struct EspaceStateValues {
    native: NativeBalances,
    standards: StandardStateValues,
}

pub(crate) fn read_espace_state_values(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    analysis_input: &EspaceAnalysisInput,
    state_phase: StatePhase,
) -> Result<EspaceStateValues, ConfluxSimulationError> {
    let native = analysis_input.native.read_state(state, state_phase)?;
    let standards = read_standard_state_values(
        state,
        machine,
        prepared_execution,
        state_phase,
        &analysis_input.standard_state_requirements,
    )?;

    Ok(EspaceStateValues { native, standards })
}

pub(crate) fn analyze_espace_changes(
    state: &mut State,
    machine: &Machine,
    prepared_execution: &PreparedTransactionExecution,
    analysis_input: EspaceAnalysisInput,
    phase_values: StatePhaseValues<EspaceStateValues>,
) -> Result<Vec<Change>, ConfluxSimulationError> {
    let EspaceAnalysisInput {
        native,
        standard_candidates,
        ..
    } = analysis_input;
    let StatePhaseValues { before, after } = phase_values;

    let standard_changes = verify(&standard_candidates, &before.standards, &after.standards)?;
    let metadata_requests = MetadataRequests::from_changes(&standard_changes);
    let mut positioned_changes = native.verify(&before.native, &after.native)?;
    positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));
    sort_changes_by_position(&mut positioned_changes);

    if positioned_changes.is_empty() {
        return Ok(Vec::new());
    }

    let metadata = load_change_metadata(state, machine, prepared_execution, &metadata_requests)?;
    Ok(into_enriched_changes(positioned_changes, &metadata))
}

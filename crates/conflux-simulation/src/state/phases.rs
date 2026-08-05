use cfx_executor::state::State;
use contract_standards::StatePhase;

use crate::ConfluxSimulationError;

#[derive(Debug)]
pub(crate) struct StatePhaseValues<T> {
    pub(crate) before: T,
    pub(crate) after: T,
}

pub(crate) fn execute_with_state_phases<Execution, AnalysisInput, StateValue>(
    state: &mut State,
    execute: impl FnOnce(&mut State) -> Result<Execution, ConfluxSimulationError>,
    prepare_analysis: impl FnOnce(&Execution) -> Result<Option<AnalysisInput>, ConfluxSimulationError>,
    mut read: impl FnMut(
        &mut State,
        &Execution,
        &AnalysisInput,
        StatePhase,
    ) -> Result<StateValue, ConfluxSimulationError>,
) -> Result<
    (
        Execution,
        Option<(AnalysisInput, StatePhaseValues<StateValue>)>,
    ),
    ConfluxSimulationError,
> {
    let before_execution_snapshot = state.save();
    let execution = execute(state)?;

    let Some(analysis_input) = prepare_analysis(&execution)? else {
        return Ok((execution, None));
    };

    let after_execution_snapshot = state.save();

    state.restore(before_execution_snapshot);
    let before = read(state, &execution, &analysis_input, StatePhase::Before)?;
    state.restore(after_execution_snapshot);

    let after_read_snapshot = state.save();
    let after = read(state, &execution, &analysis_input, StatePhase::After)?;
    state.restore(after_read_snapshot);

    Ok((
        execution,
        Some((analysis_input, StatePhaseValues { before, after })),
    ))
}

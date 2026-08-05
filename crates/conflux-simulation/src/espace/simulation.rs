use cfx_types::Space;
use contract_standards::{
    MetadataRequests, StandardCandidate, StateRequirements, state_requirements, verify,
};
use simulation_changes::{PositionedChange, into_enriched_changes, sort_changes_by_position};
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxTransactionExecutor, ObservationObserver, TransactionExecutionOutcome,
        build_conflux_state, build_mainnet_machine,
    },
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
    standards::{collect_standard_candidates, load_change_metadata, read_standard_state_values},
    state::execute_with_state_phases,
};

use super::{
    EspaceSimulation, build_espace_execution,
    changes::{
        NativeOperations, collect_native_operations, read_native_balances, verify_native_changes,
    },
};

struct EspaceAnalysisInput {
    native_operations: NativeOperations,
    standard_candidates: Vec<StandardCandidate>,
    standard_state_requirements: StateRequirements,
    execution_fee: alloy_primitives::U256,
    burnt_fee: Option<alloy_primitives::U256>,
}

pub(crate) fn simulate(
    prepared_simulation: PreparedEspaceSimulation,
    runtime_handle: &Handle,
) -> Result<EspaceSimulation, ConfluxSimulationError> {
    match prepared_simulation.state {
        PreparedEspaceSimulationState::Finished(espace_execution) => {
            Ok(EspaceSimulation::new(*espace_execution, Vec::new()))
        }
        PreparedEspaceSimulationState::Ready(ready_simulation) => {
            let ReadyEspaceSimulation {
                chain_id,
                simulated_block,
                gas_limit,
                execution_input,
                state_source,
            } = *ready_simulation;
            let mut state =
                build_conflux_state(state_source, runtime_handle.clone()).map_err(|error| {
                    ConfluxSimulationError::StateAccess {
                        message: error.to_string(),
                    }
                })?;
            let machine = build_mainnet_machine();
            let (execution, phase_values) = execute_with_state_phases(
                &mut state,
                |state| {
                    ConfluxTransactionExecutor::new(state, &machine)
                        .execute(execution_input, ObservationObserver::new(Space::Ethereum))
                        .map_err(ConfluxSimulationError::from)
                },
                |execution| {
                    let TransactionExecutionOutcome::Success(details) = &execution.outcome else {
                        return Ok(None);
                    };
                    let native_operations = collect_native_operations(&details.observations)?;
                    let standard_candidates =
                        collect_standard_candidates(&details.observations, Space::Ethereum)?;
                    let standard_state_requirements = state_requirements(&standard_candidates);
                    Ok(Some(EspaceAnalysisInput {
                        native_operations,
                        standard_candidates,
                        standard_state_requirements,
                        execution_fee: details.common.fee,
                        burnt_fee: details.common.burnt_fee,
                    }))
                },
                |state, execution, input, state_phase| {
                    let native_balances =
                        read_native_balances(state, state_phase, &input.native_operations)?;
                    let standard_state = read_standard_state_values(
                        state,
                        &machine,
                        &execution.prepared,
                        state_phase,
                        &input.standard_state_requirements,
                    )?;
                    Ok((native_balances, standard_state))
                },
            )?;

            let prepared_execution = execution.prepared;
            let transaction_outcome = execution.outcome;
            let Some((analysis_input, phase_values)) = phase_values else {
                let espace_execution = build_espace_execution(
                    chain_id,
                    simulated_block,
                    gas_limit,
                    transaction_outcome,
                )?;
                return Ok(EspaceSimulation::new(espace_execution, Vec::new()));
            };
            let EspaceAnalysisInput {
                native_operations,
                standard_candidates,
                execution_fee,
                burnt_fee,
                ..
            } = analysis_input;
            let (before_native_balances, before_standard_state) = phase_values.before;
            let (after_native_balances, after_standard_state) = phase_values.after;

            let standard_changes = verify(
                &standard_candidates,
                &before_standard_state,
                &after_standard_state,
            )?;
            let metadata_requests = MetadataRequests::from_changes(&standard_changes);
            let mut positioned_changes = verify_native_changes(
                &native_operations,
                &before_native_balances,
                &after_native_balances,
                execution_fee,
                burnt_fee,
            )?;
            positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));
            sort_changes_by_position(&mut positioned_changes);

            let enriched_changes = if positioned_changes.is_empty() {
                Vec::new()
            } else {
                let metadata = load_change_metadata(
                    &mut state,
                    &machine,
                    &prepared_execution,
                    &metadata_requests,
                )?;
                into_enriched_changes(positioned_changes, &metadata)
            };
            let espace_execution =
                build_espace_execution(chain_id, simulated_block, gas_limit, transaction_outcome)?;

            Ok(EspaceSimulation::new(espace_execution, enriched_changes))
        }
    }
}

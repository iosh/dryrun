use cfx_types::Space;
use contract_standards::{MetadataRequests, StatePhase, state_requirements, verify};
use simulation_changes::{PositionedChange, into_enriched_changes, sort_changes_by_position};
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    execution::{
        TransactionExecutionOutcome, build_mainnet_machine, build_rpc_backed_state,
        execute_transaction, prepare_transaction_execution,
    },
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
    standards::{collect_standard_candidates, load_change_metadata, read_standard_state_values},
};

use super::{
    EspaceSimulation, build_espace_execution,
    changes::{collect_native_operations, read_native_balances, verify_native_changes},
};

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
                state_reader,
            } = *ready_simulation;
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle.clone()).map_err(|error| {
                    ConfluxSimulationError::StateAccess {
                        message: error.to_string(),
                    }
                })?;
            let machine = build_mainnet_machine();
            let prepared_execution =
                prepare_transaction_execution(&state, &machine, execution_input)?;
            let before_execution_snapshot = state.save();
            let mut transaction_outcome =
                execute_transaction(&mut state, &machine, &prepared_execution)
                    .map_err(ConfluxSimulationError::from)?;

            let (execution_observations, execution_fee, burnt_fee) = match &mut transaction_outcome
            {
                TransactionExecutionOutcome::Success(executed_details) => (
                    std::mem::take(&mut executed_details.observations),
                    executed_details.common.fee,
                    executed_details.common.burnt_fee,
                ),
                _ => {
                    let espace_execution = build_espace_execution(
                        chain_id,
                        simulated_block,
                        gas_limit,
                        transaction_outcome,
                    )?;
                    return Ok(EspaceSimulation::new(espace_execution, Vec::new()));
                }
            };
            let native_operations = collect_native_operations(&execution_observations)?;
            let standard_candidates =
                collect_standard_candidates(execution_observations, Space::Ethereum)?;

            let standard_state_requirements = state_requirements(&standard_candidates);
            let after_execution_snapshot = state.save();

            state.restore(before_execution_snapshot);
            let before_native_balances =
                read_native_balances(&state, StatePhase::Before, &native_operations)?;
            let before_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::Before,
                &standard_state_requirements,
            )?;

            state.restore(after_execution_snapshot);
            let after_native_balances =
                read_native_balances(&state, StatePhase::After, &native_operations)?;
            let after_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::After,
                &standard_state_requirements,
            )?;

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

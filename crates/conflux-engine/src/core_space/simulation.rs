use cfx_types::Space;
use contract_standards::{StatePhase, state_requirements, verify};
use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{
        TransactionExecutionOutcome, build_mainnet_machine, build_rpc_backed_state,
        execute_transaction, prepare_transaction_execution,
    },
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
    standards::{collect_standard_candidates, read_standard_state_values},
};

use super::{CoreSpaceSimulation, build_core_space_execution};

pub(crate) fn simulate(
    prepared_simulation: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxEngineError> {
    match prepared_simulation.state {
        PreparedCoreSpaceSimulationState::Finished(core_execution) => {
            Ok(CoreSpaceSimulation::new(*core_execution, Vec::new()))
        }
        PreparedCoreSpaceSimulationState::Ready(ready_simulation) => {
            let ReadyCoreSpaceSimulation {
                chain_id,
                state_anchor,
                gas_limit,
                execution_input,
                state_reader,
            } = *ready_simulation;
            let mut state =
                build_rpc_backed_state(state_reader, runtime_handle.clone()).map_err(|error| {
                    ConfluxEngineError::StateAccess {
                        message: error.to_string(),
                    }
                })?;
            let machine = build_mainnet_machine();
            let prepared_execution =
                prepare_transaction_execution(&state, &machine, execution_input)?;
            let before_snapshot = state.save();
            let mut transaction_outcome =
                execute_transaction(&mut state, &machine, &prepared_execution)
                    .map_err(ConfluxEngineError::from)?;

            let execution_observations = match &mut transaction_outcome {
                TransactionExecutionOutcome::Success(executed_details) => {
                    std::mem::take(&mut executed_details.observations)
                }
                _ => {
                    let core_execution = build_core_space_execution(
                        chain_id,
                        state_anchor,
                        gas_limit,
                        transaction_outcome,
                    );
                    return Ok(CoreSpaceSimulation::new(core_execution, Vec::new()));
                }
            };
            let standard_candidates =
                collect_standard_candidates(execution_observations, Space::Native)?;
            if standard_candidates.is_empty() {
                let core_execution = build_core_space_execution(
                    chain_id,
                    state_anchor,
                    gas_limit,
                    transaction_outcome,
                );
                return Ok(CoreSpaceSimulation::new(core_execution, Vec::new()));
            }

            let standard_state_requirements = state_requirements(&standard_candidates);
            let after_snapshot = state.save();

            state.restore(before_snapshot);
            let before_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::Before,
                &standard_state_requirements,
            )?;

            state.restore(after_snapshot);
            let after_standard_state = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::After,
                &standard_state_requirements,
            )?;

            let mut positioned_standard_changes = verify(
                &standard_candidates,
                &before_standard_state,
                &after_standard_state,
            )?;
            positioned_standard_changes.sort_by_key(|change| change.position);
            let standard_changes = positioned_standard_changes
                .into_iter()
                .map(|change| change.change)
                .collect();

            let core_execution =
                build_core_space_execution(chain_id, state_anchor, gas_limit, transaction_outcome);

            Ok(CoreSpaceSimulation::new(core_execution, standard_changes))
        }
    }
}

use contract_standards::{StatePhase, state_requirements, verify};
use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{
        TransactionExecutionOutcome, build_mainnet_machine, build_rpc_backed_state,
        execute_transaction, prepare_transaction_execution,
    },
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
};

use super::{
    EspaceSimulation, build_espace_execution,
    standards::{collect_standard_candidates, read_standard_state_values},
};

pub(crate) fn simulate(
    prepared: PreparedEspaceSimulation,
    runtime_handle: &Handle,
) -> Result<EspaceSimulation, ConfluxEngineError> {
    match prepared.state {
        PreparedEspaceSimulationState::Finished(execution) => {
            Ok(EspaceSimulation::new(*execution, Vec::new()))
        }
        PreparedEspaceSimulationState::Ready(ready) => {
            let ReadyEspaceSimulation {
                chain_id,
                simulated_block,
                gas_limit,
                execution_input,
                state_reader,
            } = *ready;
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
            let mut outcome = execute_transaction(&mut state, &machine, &prepared_execution)
                .map_err(ConfluxEngineError::from)?;

            let observations = match &mut outcome {
                TransactionExecutionOutcome::Success(details) => {
                    std::mem::take(&mut details.observations)
                }
                _ => {
                    let execution =
                        build_espace_execution(chain_id, simulated_block, gas_limit, outcome)?;
                    return Ok(EspaceSimulation::new(execution, Vec::new()));
                }
            };
            let candidates = collect_standard_candidates(observations)?;

            if candidates.is_empty() {
                let execution =
                    build_espace_execution(chain_id, simulated_block, gas_limit, outcome)?;
                return Ok(EspaceSimulation::new(execution, Vec::new()));
            }

            let requirements = state_requirements(&candidates);
            let after_snapshot = state.save();

            state.restore(before_snapshot);
            let before = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::Before,
                &requirements,
            )?;

            state.restore(after_snapshot);
            let after = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::After,
                &requirements,
            )?;

            let mut positioned_changes = verify(&candidates, &before, &after)?;
            positioned_changes.sort_by_key(|positioned| positioned.position);
            let changes = positioned_changes
                .into_iter()
                .map(|positioned| positioned.change)
                .collect();
            let execution = build_espace_execution(chain_id, simulated_block, gas_limit, outcome)?;

            Ok(EspaceSimulation::new(execution, changes))
        }
    }
}

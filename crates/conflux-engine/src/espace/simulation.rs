use cfx_types::Space;
use contract_standards::{MetadataRequests, StatePhase, state_requirements, verify};
use simulation_changes::{PositionedChange, into_enriched_changes, sort_changes_by_position};
use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{
        TransactionExecutionOutcome, build_mainnet_machine, build_rpc_backed_state,
        execute_transaction, prepare_transaction_execution,
    },
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
    standards::{collect_standard_candidates, read_standard_state_values},
};

use super::{
    EspaceSimulation, build_espace_execution,
    changes::{
        collect_native_evidence, load_change_metadata, read_native_balances, verify_native_changes,
    },
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

            let (observations, fee, burnt_fee) = match &mut outcome {
                TransactionExecutionOutcome::Success(details) => (
                    std::mem::take(&mut details.observations),
                    details.common.fee,
                    details.common.burnt_fee,
                ),
                _ => {
                    let execution =
                        build_espace_execution(chain_id, simulated_block, gas_limit, outcome)?;
                    return Ok(EspaceSimulation::new(execution, Vec::new()));
                }
            };
            let native_evidence = collect_native_evidence(&observations)?;
            let candidates = collect_standard_candidates(observations, Space::Ethereum)?;

            let requirements = state_requirements(&candidates);
            let after_snapshot = state.save();

            state.restore(before_snapshot);
            let before_native = read_native_balances(&state, StatePhase::Before, &native_evidence)?;
            let before = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::Before,
                &requirements,
            )?;

            state.restore(after_snapshot);
            let after_native = read_native_balances(&state, StatePhase::After, &native_evidence)?;
            let after = read_standard_state_values(
                &mut state,
                &machine,
                &prepared_execution,
                StatePhase::After,
                &requirements,
            )?;

            let standard_changes = verify(&candidates, &before, &after)?;
            let metadata_requests = MetadataRequests::from_changes(&standard_changes);
            let mut positioned_changes = verify_native_changes(
                &native_evidence,
                &before_native,
                &after_native,
                fee,
                burnt_fee,
            )?;
            positioned_changes.extend(standard_changes.into_iter().map(PositionedChange::from));
            sort_changes_by_position(&mut positioned_changes);

            let changes = if positioned_changes.is_empty() {
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
            let execution = build_espace_execution(chain_id, simulated_block, gas_limit, outcome)?;

            Ok(EspaceSimulation::new(execution, changes))
        }
    }
}

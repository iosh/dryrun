use cfx_types::Space;
use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    execution::{
        ConfluxTransactionExecutor, ObservationObserver, TransactionExecutionOutcome,
        build_conflux_state, build_mainnet_machine,
    },
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
    state::execute_with_state_phases,
};

use super::{
    EspaceSimulation,
    analysis::{EspaceAnalysisInput, analyze_espace_changes, read_espace_state_values},
    build_espace_execution,
};

#[derive(Clone)]
pub struct EspaceSimulator {
    runtime_handle: Handle,
}

impl EspaceSimulator {
    pub fn new(runtime_handle: Handle) -> Self {
        Self { runtime_handle }
    }

    pub fn simulate(
        &self,
        prepared_simulation: PreparedEspaceSimulation,
    ) -> Result<EspaceSimulation, ConfluxSimulationError> {
        match prepared_simulation.state {
            PreparedEspaceSimulationState::Finished(espace_execution) => {
                Ok(EspaceSimulation::new(*espace_execution, Vec::new()))
            }
            PreparedEspaceSimulationState::Ready(ready_simulation) => {
                self.simulate_ready(*ready_simulation)
            }
        }
    }

    fn simulate_ready(
        &self,
        ready_simulation: ReadyEspaceSimulation,
    ) -> Result<EspaceSimulation, ConfluxSimulationError> {
        let ReadyEspaceSimulation {
            chain_id,
            simulated_block,
            gas_limit,
            execution_input,
            state_source,
        } = ready_simulation;
        let mut state =
            build_conflux_state(state_source, self.runtime_handle.clone()).map_err(|error| {
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
                if !matches!(&execution.outcome, TransactionExecutionOutcome::Success(_)) {
                    return Ok(None);
                }

                EspaceAnalysisInput::from_execution(execution).map(Some)
            },
            |state, execution, analysis_input, state_phase| {
                read_espace_state_values(
                    state,
                    &machine,
                    &execution.prepared,
                    analysis_input,
                    state_phase,
                )
            },
        )?;

        let Some((analysis_input, phase_values)) = phase_values else {
            let espace_execution =
                build_espace_execution(chain_id, simulated_block, gas_limit, execution.outcome)?;
            return Ok(EspaceSimulation::new(espace_execution, Vec::new()));
        };

        let changes = analyze_espace_changes(
            &mut state,
            &machine,
            &execution.prepared,
            analysis_input,
            phase_values,
        )?;
        let espace_execution =
            build_espace_execution(chain_id, simulated_block, gas_limit, execution.outcome)?;

        Ok(EspaceSimulation::new(espace_execution, changes))
    }
}

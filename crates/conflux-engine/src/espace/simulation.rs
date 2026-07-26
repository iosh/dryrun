use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{build_mainnet_machine, build_rpc_backed_state, execute_transaction},
    preparation::{PreparedEspaceSimulation, PreparedEspaceSimulationState, ReadyEspaceSimulation},
};

use super::{EspaceExecution, build_espace_execution};

pub(crate) fn simulate(
    prepared: PreparedEspaceSimulation,
    runtime_handle: &Handle,
) -> Result<EspaceExecution, ConfluxEngineError> {
    match prepared.kind {
        PreparedEspaceSimulationState::Complete(execution) => Ok(*execution),
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
            let outcome = execute_transaction(&mut state, &machine, execution_input)
                .map_err(ConfluxEngineError::from)?;

            build_espace_execution(chain_id, simulated_block, gas_limit, outcome)
        }
    }
}

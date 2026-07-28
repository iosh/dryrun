use tokio::runtime::Handle;

use crate::{
    ConfluxEngineError,
    execution::{build_mainnet_machine, build_rpc_backed_state, execute_transaction},
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
};

use super::{CoreSpaceExecution, build_core_space_execution};

pub(crate) fn simulate(
    prepared: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceExecution, ConfluxEngineError> {
    match prepared.state {
        PreparedCoreSpaceSimulationState::Finished(execution) => Ok(*execution),
        PreparedCoreSpaceSimulationState::Ready(ready) => {
            let ReadyCoreSpaceSimulation {
                chain_id,
                state_anchor,
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

            Ok(build_core_space_execution(
                chain_id,
                state_anchor,
                gas_limit,
                outcome,
            ))
        }
    }
}

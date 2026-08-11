use tokio::runtime::Handle;

use crate::{
    ConfluxSimulationError,
    preparation::{
        PreparedCoreSpaceSimulation, PreparedCoreSpaceSimulationState, ReadyCoreSpaceSimulation,
    },
};

use super::{CoreSpaceSimulation, build_core_space_execution, session::CoreSpaceExecutionSession};

pub(crate) fn simulate(
    prepared_simulation: PreparedCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
    match prepared_simulation.state {
        PreparedCoreSpaceSimulationState::Finished(finished) => {
            let finished = *finished;
            Ok(CoreSpaceSimulation::new(
                finished.context,
                finished.transaction,
                finished.execution,
                Vec::new(),
            ))
        }
        PreparedCoreSpaceSimulationState::Ready(ready_simulation) => {
            simulate_ready(*ready_simulation, runtime_handle)
        }
    }
}

fn simulate_ready(
    ready_simulation: ReadyCoreSpaceSimulation,
    runtime_handle: &Handle,
) -> Result<CoreSpaceSimulation, ConfluxSimulationError> {
    let ReadyCoreSpaceSimulation {
        backend,
        chain_id,
        public_context,
        transaction,
        storage_sponsorship,
        execution_block_context,
        state_source,
    } = ready_simulation;
    let session = CoreSpaceExecutionSession::new(&backend, state_source, runtime_handle.clone())?;
    let session_result =
        session.execute(&transaction, execution_block_context, storage_sponsorship)?;
    let gas_limit = transaction.gas_limit;
    let core_execution =
        build_core_space_execution(chain_id, public_context, gas_limit, session_result.outcome);
    Ok(CoreSpaceSimulation::new(
        public_context,
        transaction,
        core_execution,
        session_result.changes,
    ))
}

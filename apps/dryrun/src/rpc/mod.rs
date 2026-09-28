mod error;
mod request;

use std::sync::Arc;

use evm_simulation::{Outcome, Simulation, Simulator};
use jsonrpsee::{RpcModule, core::RegisterMethodError};
use simulation_core::{CodedError, ExecutionStatus};

use crate::tasks::SimulationTaskSet;
use error::{invalid_params, rpc_error};
use request::BlockRequest;

pub(crate) fn register_evm(
    module: &mut RpcModule<()>,
    simulator: Simulator,
    tasks: SimulationTaskSet,
) -> Result<(), RegisterMethodError> {
    module.register_async_method("dryrun_evm_simulateTransaction", move |params, _, _| {
        let simulator = simulator.clone();
        let tasks = tasks.clone();
        async move {
            let request = params
                .parse::<BlockRequest>()
                .map_err(invalid_params)?
                .into_evm()
                .map_err(|error| rpc_error(&error))?;
            tasks
                .run(move || async move { simulator.simulate(request).await })
                .await
                .map_err(|error| rpc_error(&error))?
                .map(response)
                .map_err(|error| rpc_error(&error))
        }
    })?;
    Ok(())
}

fn response(simulation: Simulation) -> Arc<Simulation> {
    if let Outcome::Executed(execution) = &simulation.outcome
        && let ExecutionStatus::Success {
            changes: Err(error),
            ..
        } = &execution.status
    {
        tracing::warn!(?error, code = ?error.code(), "transaction changes unavailable");
    }
    Arc::new(simulation)
}

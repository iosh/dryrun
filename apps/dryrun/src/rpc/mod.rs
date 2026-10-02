mod error;
mod request;

use crate::tasks::SimulationTaskSet;
use alloy::{eips::BlockId, rpc::types::TransactionRequest};
use error::{invalid_params, rpc_error};
use jsonrpsee::{RpcModule, core::RegisterMethodError};
use request::BlockRequest;
use serde::Serialize;
use simulation_core::{CodedError, ExecutionStatus};
use std::{fmt::Debug, future::Future, sync::Arc};

pub(crate) fn register_evm(
    module: &mut RpcModule<()>,
    simulator: evm_simulation::Simulator,
    tasks: SimulationTaskSet,
) -> Result<(), RegisterMethodError> {
    register(
        module,
        "dryrun_evm_simulateTransaction",
        tasks,
        move |block, transaction| {
            let simulator = simulator.clone();
            async move {
                let simulation = simulator
                    .simulate(evm_simulation::SimulationRequest { block, transaction })
                    .await?;
                if let evm_simulation::Outcome::Executed(execution) = &simulation.outcome {
                    log_changes_error(&execution.status);
                }
                Ok::<_, evm_simulation::Error>(simulation)
            }
        },
    )
}

pub(crate) fn register_espace(
    module: &mut RpcModule<()>,
    simulator: conflux_simulation::espace::Simulator,
    tasks: SimulationTaskSet,
) -> Result<(), RegisterMethodError> {
    let simulator = Arc::new(simulator);
    register(
        module,
        "dryrun_conflux_espace_simulateTransaction",
        tasks,
        move |block, transaction| {
            let simulator = Arc::clone(&simulator);
            async move {
                let simulation = simulator
                    .simulate(conflux_simulation::espace::SimulationRequest { block, transaction })
                    .await?;
                if let conflux_simulation::espace::Outcome::Executed(execution) =
                    &simulation.outcome
                {
                    log_changes_error(&execution.status);
                }
                Ok::<_, conflux_simulation::Error>(simulation)
            }
        },
    )
}

fn register<F, Fut, S, E>(
    module: &mut RpcModule<()>,
    name: &'static str,
    tasks: SimulationTaskSet,
    simulate: F,
) -> Result<(), RegisterMethodError>
where
    F: Fn(BlockId, TransactionRequest) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<S, E>> + Send + 'static,
    S: Serialize + Send + Sync + 'static,
    E: CodedError + Debug + Send + 'static,
{
    module.register_async_method(name, move |params, _, _| {
        let simulate = simulate.clone();
        let tasks = tasks.clone();
        async move {
            let (block, transaction) = params
                .parse::<BlockRequest>()
                .map_err(invalid_params)?
                .into_parts()?;
            tasks
                .run(move || simulate(block, transaction))
                .await
                .map_err(|error| rpc_error(&error))?
                .map(Arc::new)
                .map_err(|error| rpc_error(&error))
        }
    })?;
    Ok(())
}

fn log_changes_error<E: CodedError + Debug>(
    status: &ExecutionStatus<alloy::primitives::Address, E>,
) {
    if let ExecutionStatus::Success {
        changes: Err(error),
        ..
    } = status
    {
        tracing::warn!(?error, code = ?error.code(), "transaction changes unavailable");
    }
}

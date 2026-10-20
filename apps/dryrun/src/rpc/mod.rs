mod error;
mod request;
mod server;

pub(crate) use server::start;

use crate::tasks::SimulationTaskSet;
use error::{invalid_params, rpc_error};
use jsonrpsee::{RpcModule, core::RegisterMethodError, types::ErrorObjectOwned};
use request::{BlockRequest, EpochRequest, Request};
use serde::Serialize;
use simulation_core::{CodedError, ExecutionStatus, Outcome};
use std::{
    fmt::Debug,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tracing::Instrument;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn register_evm(
    module: &mut RpcModule<SimulationTaskSet>,
    simulator: evm_simulation::Simulator,
) -> Result<(), RegisterMethodError> {
    register_simulation::<BlockRequest, _, _, _, _>(
        module,
        "dryrun_evm_simulateTransaction",
        move |block, transaction| {
            let simulator = simulator.clone();
            async move {
                let simulation = simulator
                    .simulate(evm_simulation::SimulationRequest { block, transaction })
                    .await?;
                if let Outcome::Executed(execution) = &simulation.outcome {
                    log_changes_error(&execution.status);
                }
                Ok::<_, evm_simulation::Error>(simulation)
            }
        },
    )
}

fn register_espace(
    module: &mut RpcModule<SimulationTaskSet>,
    simulator: conflux_simulation::espace::Simulator,
) -> Result<(), RegisterMethodError> {
    let simulator = Arc::new(simulator);
    register_simulation::<BlockRequest, _, _, _, _>(
        module,
        "dryrun_conflux_espace_simulateTransaction",
        move |block, transaction| {
            let simulator = Arc::clone(&simulator);
            async move {
                let simulation = simulator
                    .simulate(conflux_simulation::espace::SimulationRequest { block, transaction })
                    .await?;
                if let Outcome::Executed(execution) = &simulation.outcome {
                    log_changes_error(&execution.status);
                }
                Ok::<_, conflux_simulation::Error>(simulation)
            }
        },
    )
}

fn register_core(
    module: &mut RpcModule<SimulationTaskSet>,
    simulator: conflux_simulation::core_space::Simulator,
) -> Result<(), RegisterMethodError> {
    let simulator = Arc::new(simulator);
    register_simulation::<EpochRequest, _, _, _, _>(
        module,
        "dryrun_conflux_core_simulateTransaction",
        move |epoch, transaction| {
            let simulator = Arc::clone(&simulator);
            async move {
                let simulation = simulator
                    .simulate(conflux_simulation::core_space::SimulationRequest {
                        epoch,
                        transaction,
                    })
                    .await?;
                if let Outcome::Executed(execution) = &simulation.outcome {
                    log_changes_error(&execution.status);
                }
                Ok::<_, conflux_simulation::Error>(simulation)
            }
        },
    )
}

fn register_simulation<R, F, Fut, S, E>(
    module: &mut RpcModule<SimulationTaskSet>,
    name: &'static str,
    simulate: F,
) -> Result<(), RegisterMethodError>
where
    R: Request,
    F: Fn(R::Selector, R::Transaction) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<S, E>> + Send + 'static,
    S: Serialize + Send + Sync + 'static,
    E: CodedError + Debug + Send + 'static,
{
    module.register_async_method(name, move |params, tasks, _| {
        let simulate = simulate.clone();
        let span = tracing::info_span!(
            "simulation_request",
            method = name,
            request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
        );
        async move {
            let request = params.parse::<R>().map_err(invalid_params)?;
            let (selector, transaction) = request.into_parts()?;
            let simulation = tasks
                .run(
                    move || async move { simulate(selector, transaction).await.map_err(rpc_error) },
                )
                .await
                .map_err(rpc_error)??;
            Ok::<_, ErrorObjectOwned>(Arc::new(simulation))
        }
        .instrument(span)
    })?;
    Ok(())
}

fn log_changes_error<C, E: CodedError + Debug>(status: &ExecutionStatus<C, E>) {
    if let ExecutionStatus::Success {
        changes: Err(error),
        ..
    } = status
    {
        tracing::warn!(?error, code = ?error.code(), "transaction changes unavailable");
    }
}

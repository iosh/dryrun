mod error;
mod request;

use std::{fmt::Debug, sync::Arc};

use conflux_simulation::{
    core_space::CoreSpaceTransactionSimulator, espace::EspaceTransactionSimulator,
};
use evm_simulation::EvmTransactionSimulator;
use jsonrpsee::{RpcModule, core::RegisterMethodError};
use simulation_core::{
    error::ErrorInfo,
    simulation::{Changes, Simulation},
};

use crate::simulation_tasks::SimulationTaskSet;
use error::{invalid_params, rpc_error};
use request::{BlockRequest, CoreRequest};

pub(crate) fn register_evm(
    module: &mut RpcModule<()>,
    simulator: EvmTransactionSimulator,
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
                .map_err(rpc_error)?;
            tasks
                .run(move || async move {
                    simulator
                        .simulate(request)
                        .await
                        .map(simulation_response)
                        .map_err(rpc_error)
                })
                .await
                .map_err(rpc_error)?
        }
    })?;
    Ok(())
}

pub(crate) fn register_conflux(
    module: &mut RpcModule<()>,
    espace: EspaceTransactionSimulator,
    core: CoreSpaceTransactionSimulator,
    tasks: SimulationTaskSet,
) -> Result<(), RegisterMethodError> {
    let espace_tasks = tasks.clone();
    module.register_async_method(
        "dryrun_conflux_espace_simulateTransaction",
        move |params, _, _| {
            let simulator = espace.clone();
            let tasks = espace_tasks.clone();
            async move {
                let request = params
                    .parse::<BlockRequest>()
                    .map_err(invalid_params)?
                    .into_espace()
                    .map_err(rpc_error)?;
                tasks
                    .run(move || async move {
                        simulator
                            .simulate(request)
                            .await
                            .map(simulation_response)
                            .map_err(rpc_error)
                    })
                    .await
                    .map_err(rpc_error)?
            }
        },
    )?;
    module.register_async_method(
        "dryrun_conflux_coreSpace_simulateTransaction",
        move |params, _, _| {
            let simulator = core.clone();
            let tasks = tasks.clone();
            async move {
                let request = params
                    .parse::<CoreRequest>()
                    .map_err(invalid_params)?
                    .into_simulation()
                    .map_err(rpc_error)?;
                tasks
                    .run(move || async move {
                        simulator
                            .simulate(request)
                            .await
                            .map(simulation_response)
                            .map_err(rpc_error)
                    })
                    .await
                    .map_err(rpc_error)?
            }
        },
    )?;
    Ok(())
}

fn simulation_response<C, T, P, O, R, S, E: ErrorInfo + Debug>(
    simulation: Simulation<C, T, P, O, R, S, E>,
) -> Arc<Simulation<C, T, P, O, R, S, E>> {
    if let Simulation::Executed(executed) = &simulation
        && let Changes::Unavailable(error) = executed.changes()
    {
        tracing::warn!(
            error = ?error,
            code = ?error.diagnostic().code,
            "transaction changes unavailable"
        );
    }
    Arc::new(simulation)
}

mod error;
mod request;
mod response;

use conflux_simulation::{
    core_space::CoreSpaceTransactionSimulator, espace::EspaceTransactionSimulator,
};
use jsonrpsee::{RpcModule, types::ErrorObjectOwned};
use simulation_tasks::SimulationTaskSet;

use self::{
    error::{invalid_params, rpc_error},
    request::{SimulateCoreSpaceTransactionRequest, SimulateEspaceTransactionRequest},
    response::{SimulateCoreSpaceTransactionResponse, SimulateEspaceTransactionResponse},
};

const METHOD_SIMULATE_ESPACE_TRANSACTION: &str = "dryrun_conflux_espace_simulateTransaction";
const METHOD_SIMULATE_CORE_SPACE_TRANSACTION: &str = "dryrun_conflux_coreSpace_simulateTransaction";

pub fn build_rpc_module(
    espace_simulator: EspaceTransactionSimulator,
    core_space_simulator: CoreSpaceTransactionSimulator,
    simulation_tasks: SimulationTaskSet,
) -> RpcModule<()> {
    let mut module = RpcModule::new(());
    let espace_simulation_tasks = simulation_tasks.clone();

    module
        .register_async_method(METHOD_SIMULATE_ESPACE_TRANSACTION, move |params, _, _| {
            let simulator = espace_simulator.clone();
            let simulation_tasks = espace_simulation_tasks.clone();
            async move {
                let request = params
                    .parse::<SimulateEspaceTransactionRequest>()
                    .map_err(|error| invalid_params(error.to_string()))?;
                let input = request.try_into()?;

                let output = simulation_tasks
                    .run(move || async move { simulator.simulate(input).await })
                    .await
                    .map_err(rpc_error)?
                    .map_err(rpc_error)?;

                Ok::<_, ErrorObjectOwned>(SimulateEspaceTransactionResponse::from(output))
            }
        })
        .expect("RPC method names must be unique");

    module
        .register_async_method(
            METHOD_SIMULATE_CORE_SPACE_TRANSACTION,
            move |params, _, _| {
                let simulator = core_space_simulator.clone();
                let simulation_tasks = simulation_tasks.clone();
                async move {
                    let request = params
                        .parse::<SimulateCoreSpaceTransactionRequest>()
                        .map_err(|error| invalid_params(error.to_string()))?;

                    let input = request.try_into()?;

                    let output = simulation_tasks
                        .run(move || async move { simulator.simulate(input).await })
                        .await
                        .map_err(rpc_error)?
                        .map_err(rpc_error)?;

                    Ok::<_, ErrorObjectOwned>(SimulateCoreSpaceTransactionResponse::from(output))
                }
            },
        )
        .expect("RPC method names must be unique");

    module
}

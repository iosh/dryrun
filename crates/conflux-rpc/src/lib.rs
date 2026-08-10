mod error;
mod request;
mod response;

use std::sync::Arc;

use cfx_addr::Network as RpcNetwork;
use conflux_provider::Network;
use conflux_service::ConfluxService;
use conflux_simulation::espace::EspaceTransactionSimulator;
use jsonrpsee::{RpcModule, types::ErrorObjectOwned};
use simulation_tasks::SimulationTaskSet;

use self::{
    error::{
        core_space_response_mapping_error, invalid_params, map_core_space_service_error,
        map_espace_simulation_error, map_simulation_task_error,
    },
    request::{SimulateCoreSpaceTransactionRequest, SimulateEspaceTransactionRequest},
    response::{SimulateCoreSpaceTransactionResponse, SimulateEspaceTransactionResponse},
};

const METHOD_SIMULATE_ESPACE_TRANSACTION: &str = "dryrun_conflux_espace_simulateTransaction";
const METHOD_SIMULATE_CORE_SPACE_TRANSACTION: &str = "dryrun_conflux_coreSpace_simulateTransaction";

pub fn build_rpc_module(
    espace_simulator: EspaceTransactionSimulator,
    service: Arc<ConfluxService>,
    simulation_tasks: SimulationTaskSet,
    core_space_address_network: Network,
) -> RpcModule<Arc<ConfluxService>> {
    let rpc_network = to_rpc_network(core_space_address_network);
    let mut module = RpcModule::new(service);

    module
        .register_async_method(METHOD_SIMULATE_ESPACE_TRANSACTION, move |params, _, _| {
            let simulator = espace_simulator.clone();
            let simulation_tasks = simulation_tasks.clone();
            async move {
                let request = params
                    .parse::<SimulateEspaceTransactionRequest>()
                    .map_err(|error| invalid_params(error.to_string()))?;
                let input = request.try_into()?;

                let output = simulation_tasks
                    .run(move || async move { simulator.simulate(input).await })
                    .await
                    .map_err(map_simulation_task_error)?
                    .map_err(map_espace_simulation_error)?;

                Ok::<_, ErrorObjectOwned>(SimulateEspaceTransactionResponse::from(output))
            }
        })
        .expect("RPC method names must be unique");

    module
        .register_async_method(
            METHOD_SIMULATE_CORE_SPACE_TRANSACTION,
            move |params, service, _| async move {
                let request = params
                    .parse::<SimulateCoreSpaceTransactionRequest>()
                    .map_err(|error| invalid_params(error.to_string()))?;

                let input = request.try_into_service_input(rpc_network)?;

                let output = service
                    .simulate_core_space_transaction(input)
                    .await
                    .map_err(map_core_space_service_error)?;

                SimulateCoreSpaceTransactionResponse::try_from_output(output, rpc_network)
                    .map_err(|error| core_space_response_mapping_error(error.to_string()))
            },
        )
        .expect("RPC method names must be unique");

    module
}

fn to_rpc_network(network: Network) -> RpcNetwork {
    match network {
        Network::Main => RpcNetwork::Main,
        Network::Test => RpcNetwork::Test,
        Network::Id(id) => RpcNetwork::Id(id),
    }
}

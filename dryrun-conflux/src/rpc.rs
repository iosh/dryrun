mod error;
mod request;
mod response;

use std::sync::Arc;

use cfx_addr::Network;
use conflux_service::ConfluxService;
use jsonrpsee::{RpcModule, types::ErrorObjectOwned};

use self::{
    error::{invalid_params, map_service_error, response_mapping_error},
    request::{SimulateEspaceTransactionRequest, SimulateNativeTransactionRequest},
    response::{SimulateEspaceTransactionResponse, SimulateNativeTransactionResponse},
};

const METHOD_SIMULATE_ESPACE_TRANSACTION: &str = "dryrun_conflux_espace_simulateTransaction";
const METHOD_SIMULATE_NATIVE_TRANSACTION: &str = "dryrun_conflux_native_simulateTransaction";

pub fn build_rpc_module(
    service: Arc<ConfluxService>,
    native_address_network: Network,
) -> RpcModule<Arc<ConfluxService>> {
    let mut module = RpcModule::new(service);

    module
        .register_async_method(
            METHOD_SIMULATE_ESPACE_TRANSACTION,
            |params, service, _| async move {
                let request = params
                    .parse::<SimulateEspaceTransactionRequest>()
                    .map_err(|error| invalid_params(error.to_string()))?;

                let output = service
                    .simulate_espace_transaction(
                        request.try_into().map_err(ErrorObjectOwned::from)?,
                    )
                    .map_err(|error| map_service_error(&error))?;

                Ok::<_, ErrorObjectOwned>(SimulateEspaceTransactionResponse::from(output))
            },
        )
        .expect("RPC method names must be unique");

    module
        .register_async_method(
            METHOD_SIMULATE_NATIVE_TRANSACTION,
            move |params, service, _| async move {
                let request = params
                    .parse::<SimulateNativeTransactionRequest>()
                    .map_err(|error| invalid_params(error.to_string()))?;

                let input = request
                    .try_into_service_input(native_address_network)
                    .map_err(ErrorObjectOwned::from)?;

                let output = service
                    .simulate_native_transaction(input)
                    .map_err(|error| map_service_error(&error))?;

                SimulateNativeTransactionResponse::try_from_output(output, native_address_network)
                    .map_err(|error| response_mapping_error(error.to_string()))
            },
        )
        .expect("RPC method names must be unique");

    module
}

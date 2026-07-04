mod error;
mod request;
mod response;

use std::sync::Arc;

use conflux_service::ConfluxService;
use jsonrpsee::{RpcModule, types::ErrorObjectOwned};

use self::{
    error::{invalid_params, map_service_error},
    request::SimulateEspaceTransactionRequest,
    response::SimulateEspaceTransactionResponse,
};

const METHOD_SIMULATE_ESPACE_TRANSACTION: &str = "dryrun_conflux_espace_simulateTransaction";

pub fn build_rpc_module(service: Arc<ConfluxService>) -> RpcModule<Arc<ConfluxService>> {
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
}

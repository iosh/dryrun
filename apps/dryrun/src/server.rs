use std::{io, time::Duration};

use alloy::{
    providers::{Provider, RootProvider},
    rpc::client::RpcClient,
    transports::http::reqwest::Client as HttpClient,
};
use evm_simulation::{ChainSpec, Simulator};
use jsonrpsee::{
    RpcModule,
    server::{BatchRequestConfig, Server, ServerConfig as JsonRpcServerConfig, ServerHandle},
    types::ErrorObjectOwned,
};
use tracing::info;

use crate::{config::AppConfig, rpc, tasks::SimulationTaskSet};

const MAX_RPC_CONNECTIONS: u32 = 100;
const MAX_RPC_BODY_SIZE_BYTES: u32 = 10 * 1024 * 1024;
const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn start(config: &AppConfig, tasks: SimulationTaskSet) -> io::Result<ServerHandle> {
    let module = build_rpc_module(config, tasks).await?;
    let server_config = JsonRpcServerConfig::builder()
        .max_connections(MAX_RPC_CONNECTIONS)
        .max_request_body_size(MAX_RPC_BODY_SIZE_BYTES)
        .max_response_body_size(MAX_RPC_BODY_SIZE_BYTES)
        .set_batch_request_config(BatchRequestConfig::Disabled)
        .build();
    let server = Server::builder()
        .set_config(server_config)
        .build(format!("{}:{}", config.server.host, config.server.port))
        .await?;
    let address = server.local_addr()?;
    let handle = server.start(module);

    info!("RPC server started at {address}");

    Ok(handle)
}

async fn build_rpc_module(
    config: &AppConfig,
    tasks: SimulationTaskSet,
) -> io::Result<RpcModule<()>> {
    let http_client = HttpClient::builder()
        .timeout(PROVIDER_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            startup_error(format!("failed to configure provider HTTP client: {error}"))
        })?;
    let rpc_url = config.ethereum.rpc_url.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Ethereum RPC URL: {error}"),
        )
    })?;
    let provider =
        RootProvider::new(RpcClient::new_http_with_client(http_client, rpc_url)).erased();
    let simulator = Simulator::new(provider, ChainSpec::mainnet(), config.ethereum.limits)
        .await
        .map_err(|error| {
            startup_error(format!("failed to initialize Ethereum simulation: {error}"))
        })?;

    let mut module = RpcModule::new(());
    rpc::register_evm(&mut module, simulator, tasks)
        .map_err(|error| startup_error(format!("failed to register EVM RPC method: {error}")))?;
    module
        .register_method("dryrun_health", |_, _, _| Ok::<_, ErrorObjectOwned>("ok"))
        .map_err(|error| startup_error(format!("failed to register health RPC method: {error}")))?;
    Ok(module)
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

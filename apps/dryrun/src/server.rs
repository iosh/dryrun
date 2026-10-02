use std::{io, time::Duration};

use alloy::{
    providers::{Provider, RootProvider},
    rpc::client::RpcClient,
    transports::http::reqwest::Client as HttpClient,
};
use jsonrpsee::{
    RpcModule,
    server::{BatchRequestConfig, Server, ServerConfig as JsonRpcServerConfig, ServerHandle},
    types::ErrorObjectOwned,
};
use tracing::info;

use crate::{
    config::{AppConfig, ConfluxConfig, EthereumConfig},
    rpc,
    tasks::SimulationTaskSet,
};

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
        .map_err(io::Error::other)?;
    let mut module = RpcModule::new(());

    let ethereum = build_ethereum_simulator(&config.ethereum, &http_client).await?;
    rpc::register_evm(&mut module, ethereum, tasks.clone()).map_err(io::Error::other)?;

    if let Some(conflux) = &config.conflux {
        let espace = build_espace_simulator(conflux, &http_client).await?;
        rpc::register_espace(&mut module, espace, tasks).map_err(io::Error::other)?;
    }

    module
        .register_method("dryrun_health", |_, _, _| Ok::<_, ErrorObjectOwned>("ok"))
        .map_err(io::Error::other)?;
    Ok(module)
}

async fn build_ethereum_simulator(
    config: &EthereumConfig,
    http_client: &HttpClient,
) -> io::Result<evm_simulation::Simulator> {
    let client = RpcClient::new_http_with_client(http_client.clone(), config.rpc_url.clone());
    let provider = RootProvider::new(client).erased();
    evm_simulation::Simulator::new(
        provider,
        evm_simulation::ChainSpec::mainnet(),
        config.limits,
    )
    .await
    .map_err(io::Error::other)
}

async fn build_espace_simulator(
    config: &ConfluxConfig,
    http_client: &HttpClient,
) -> io::Result<conflux_simulation::espace::Simulator> {
    let core = conflux_provider::ConfluxProvider::new(RpcClient::new_http_with_client(
        http_client.clone(),
        config.core_rpc_url.clone(),
    ));
    let espace = RootProvider::new(RpcClient::new_http_with_client(
        http_client.clone(),
        config.espace_rpc_url.clone(),
    ))
    .erased();
    conflux_simulation::espace::Simulator::new(
        core,
        espace,
        conflux_simulation::ChainSpec::mainnet(),
        config.limits,
    )
    .await
    .map_err(io::Error::other)
}

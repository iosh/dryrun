use std::{io, sync::Arc};

use alloy::providers::{DynProvider, Provider, ProviderBuilder};
use conflux_engine::{ConfluxEngine, config::ConfluxChainConfig, state::HttpConfluxStateProvider};
use conflux_rpc::build_rpc_module as build_conflux_rpc_module;
use conflux_service::ConfluxService;
use evm_engine::EvmEngine;
use evm_rpc::{DryrunRpcServer, RpcHandler};
use evm_service::SimulationService;
use jsonrpsee::{
    RpcModule,
    server::{BatchRequestConfig, Server, ServerConfig as JsonRpcServerConfig, ServerHandle},
    types::ErrorObjectOwned,
};
use simulation_tasks::SimulationTaskSet;
use tracing::info;

use crate::app_config::{AppConfig, ConfluxConfig, EthereumConfig};

const MAX_RPC_CONNECTIONS: u32 = 100;
const MAX_RPC_BODY_SIZE_BYTES: u32 = 10 * 1024 * 1024;

pub async fn start(
    config: &AppConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<ServerHandle> {
    let rpc_module = build_host_rpc_module(config, simulation_tasks)?;
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
    let rpc_handle = server.start(rpc_module);

    info!("RPC server started at {address}");

    Ok(rpc_handle)
}

fn build_host_rpc_module(
    config: &AppConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<RpcModule<()>> {
    let mut rpc_module = RpcModule::new(());

    add_evm_rpc_module(&mut rpc_module, &config.ethereum, simulation_tasks.clone())?;
    add_conflux_rpc_module(&mut rpc_module, &config.conflux, simulation_tasks)?;
    rpc_module
        .register_method("dryrun_health", |_, _, _| Ok::<_, ErrorObjectOwned>("ok"))
        .map_err(|error| startup_error(format!("failed to register health RPC method: {error}")))?;

    Ok(rpc_module)
}

fn add_evm_rpc_module(
    rpc_module: &mut RpcModule<()>,
    config: &EthereumConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<()> {
    let ethereum_provider = create_ethereum_provider(config)?;
    let evm_engine = Arc::new(EvmEngine::new(
        ethereum_provider.clone(),
        tokio::runtime::Handle::current(),
    ));
    let simulation_service = Arc::new(SimulationService::new(
        ethereum_provider,
        evm_engine,
        simulation_tasks,
    ));

    rpc_module
        .merge(RpcHandler::new(simulation_service).into_rpc())
        .map_err(|error| startup_error(format!("failed to merge EVM RPC module: {error}")))
}

fn add_conflux_rpc_module(
    rpc_module: &mut RpcModule<()>,
    config: &ConfluxConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<()> {
    let conflux_chain = ConfluxChainConfig::mainnet();
    let core_space_address_network = conflux_chain.core_space_address_network;
    let conflux_provider = Arc::new(create_conflux_provider(config, &conflux_chain)?);
    let conflux_engine = Arc::new(ConfluxEngine::new(conflux_chain, conflux_provider));
    let conflux_service = Arc::new(ConfluxService::new(conflux_engine, simulation_tasks));

    rpc_module
        .merge(build_conflux_rpc_module(
            conflux_service,
            core_space_address_network,
        ))
        .map_err(|error| startup_error(format!("failed to merge Conflux RPC module: {error}")))
}

fn create_ethereum_provider(config: &EthereumConfig) -> io::Result<DynProvider> {
    let rpc_url: reqwest::Url = config
        .rpc_url
        .parse()
        .map_err(|error| configuration_error(format!("invalid Ethereum RPC URL: {error}")))?;
    let client = reqwest::Client::builder().build().map_err(|error| {
        startup_error(format!("failed to create Ethereum HTTP client: {error}"))
    })?;

    Ok(ProviderBuilder::new()
        .connect_reqwest(client, rpc_url)
        .erased())
}

fn create_conflux_provider(
    config: &ConfluxConfig,
    chain: &ConfluxChainConfig,
) -> io::Result<HttpConfluxStateProvider> {
    HttpConfluxStateProvider::new(
        &config.espace_rpc_url,
        &config.core_space_rpc_url,
        chain.core_space_address_network,
    )
    .map_err(|error| {
        configuration_error(format!("failed to create Conflux state provider: {error}"))
    })
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn configuration_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

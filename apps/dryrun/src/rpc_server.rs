use std::{io, sync::Arc};

use alloy::providers::{Provider, RootProvider};
use alloy_rpc_client::RpcClient;
use conflux_provider::ConfluxProvider;
use conflux_rpc::build_rpc_module as build_conflux_rpc_module;
use conflux_service::ConfluxService;
use conflux_simulation::{
    ConfluxSimulationProvider,
    config::ConfluxChainConfig,
    core_space::{CoreSpaceSimulationPreparer, CoreSpaceSimulator},
    espace::{EspaceSimulationPreparer, EspaceSimulator},
};
use evm_rpc::{DryrunRpcServer, RpcHandler};
use evm_simulation::EvmTransactionSimulator;
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
    let rpc_module = build_host_rpc_module(config, simulation_tasks).await?;
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

async fn build_host_rpc_module(
    config: &AppConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<RpcModule<()>> {
    let mut rpc_module = RpcModule::new(());

    add_evm_rpc_module(&mut rpc_module, &config.ethereum, simulation_tasks.clone()).await?;
    add_conflux_rpc_module(&mut rpc_module, &config.conflux, simulation_tasks)?;
    rpc_module
        .register_method("dryrun_health", |_, _, _| Ok::<_, ErrorObjectOwned>("ok"))
        .map_err(|error| startup_error(format!("failed to register health RPC method: {error}")))?;

    Ok(rpc_module)
}

async fn add_evm_rpc_module(
    rpc_module: &mut RpcModule<()>,
    config: &EthereumConfig,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<()> {
    let ethereum_provider = create_ethereum_provider(config)?.erased();
    let evm_simulator = EvmTransactionSimulator::ethereum_mainnet(ethereum_provider)
        .await
        .map_err(|error| {
            startup_error(format!("failed to initialize Ethereum simulation: {error}"))
        })?;

    rpc_module
        .merge(RpcHandler::new(evm_simulator, simulation_tasks).into_rpc())
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
    let runtime_handle = tokio::runtime::Handle::current();
    let espace_preparer = Arc::new(EspaceSimulationPreparer::new(
        conflux_chain.clone(),
        Arc::clone(&conflux_provider),
    ));
    let espace_simulator = Arc::new(EspaceSimulator::new(runtime_handle.clone()));
    let core_space_preparer = Arc::new(CoreSpaceSimulationPreparer::new(
        conflux_chain,
        Arc::clone(&conflux_provider),
    ));
    let core_space_simulator = Arc::new(CoreSpaceSimulator::new(runtime_handle));
    let conflux_service = Arc::new(ConfluxService::new(
        espace_preparer,
        espace_simulator,
        core_space_preparer,
        core_space_simulator,
        simulation_tasks,
    ));

    rpc_module
        .merge(build_conflux_rpc_module(
            conflux_service,
            core_space_address_network,
        ))
        .map_err(|error| startup_error(format!("failed to merge Conflux RPC module: {error}")))
}

fn create_ethereum_provider(config: &EthereumConfig) -> io::Result<RootProvider> {
    let rpc_url = config
        .rpc_url
        .parse()
        .map_err(|error| configuration_error(format!("invalid Ethereum RPC URL: {error}")))?;

    Ok(RootProvider::new_http(rpc_url))
}

fn create_conflux_provider(
    config: &ConfluxConfig,
    chain: &ConfluxChainConfig,
) -> io::Result<ConfluxSimulationProvider> {
    let espace_provider = RootProvider::new_http(
        config
            .espace_rpc_url
            .parse()
            .map_err(|error| configuration_error(format!("invalid eSpace RPC URL: {error}")))?,
    );
    let core_space_provider = Arc::new(ConfluxProvider::new(RpcClient::new_http(
        config
            .core_space_rpc_url
            .parse()
            .map_err(|error| configuration_error(format!("invalid Core Space RPC URL: {error}")))?,
    )));
    Ok(ConfluxSimulationProvider::new(
        espace_provider,
        core_space_provider,
        chain.core_space_address_network,
    ))
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn configuration_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

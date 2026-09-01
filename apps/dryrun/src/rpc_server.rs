use std::{io, time::Duration};

use alloy::{
    providers::{Provider, RootProvider},
    transports::http::reqwest::Client as HttpClient,
};
use alloy_rpc_client::RpcClient;
use conflux_provider::ConfluxProvider;
use conflux_rpc::build_rpc_module as build_conflux_rpc_module;
use conflux_simulation::{
    ConfluxSimulationBackend, core_space::CoreSpaceTransactionSimulator,
    espace::EspaceTransactionSimulator,
};
use evm_rpc::{DryrunRpcServer, RpcHandler};
use evm_simulation::{EvmSimulationLimits, EvmTransactionSimulator};
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
const PROVIDER_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    let http_client = create_provider_http_client()?;

    add_evm_rpc_module(
        &mut rpc_module,
        &config.ethereum,
        &http_client,
        simulation_tasks.clone(),
    )
    .await?;
    add_conflux_rpc_module(
        &mut rpc_module,
        &config.conflux,
        &http_client,
        simulation_tasks,
    )
    .await?;
    rpc_module
        .register_method("dryrun_health", |_, _, _| Ok::<_, ErrorObjectOwned>("ok"))
        .map_err(|error| startup_error(format!("failed to register health RPC method: {error}")))?;

    Ok(rpc_module)
}

async fn add_evm_rpc_module(
    rpc_module: &mut RpcModule<()>,
    config: &EthereumConfig,
    http_client: &HttpClient,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<()> {
    let ethereum_provider = create_ethereum_provider(config, http_client)?.erased();
    let evm_simulator = EvmTransactionSimulator::ethereum_mainnet(ethereum_provider)
        .await
        .map_err(|error| {
            startup_error(format!("failed to initialize Ethereum simulation: {error}"))
        })?
        .with_limits(EvmSimulationLimits {
            max_occurrence_checkpoints: config.limits.max_occurrence_checkpoints,
            max_retained_state_entries: config.limits.max_retained_state_entries,
            max_state_reads: config.limits.max_state_reads,
            max_read_calls: config.limits.max_read_calls,
            read_call_gas_limit: config.limits.read_call_gas_limit,
            max_read_call_output_bytes: config.limits.max_read_call_output_bytes,
        });

    rpc_module
        .merge(RpcHandler::new(evm_simulator, simulation_tasks).into_rpc())
        .map_err(|error| startup_error(format!("failed to merge EVM RPC module: {error}")))
}

async fn add_conflux_rpc_module(
    rpc_module: &mut RpcModule<()>,
    config: &ConfluxConfig,
    http_client: &HttpClient,
    simulation_tasks: SimulationTaskSet,
) -> io::Result<()> {
    let (espace_provider, core_space_provider) = create_conflux_providers(config, http_client)?;
    let backend = ConfluxSimulationBackend::mainnet(espace_provider.erased(), core_space_provider)
        .await
        .map_err(|error| {
            startup_error(format!("failed to initialize Conflux simulation: {error}"))
        })?;
    let core_space_address_network = backend.core_space_address_network();
    let espace_simulator = EspaceTransactionSimulator::new(backend.clone());
    let core_space_simulator = CoreSpaceTransactionSimulator::new(backend);

    rpc_module
        .merge(build_conflux_rpc_module(
            espace_simulator,
            core_space_simulator,
            simulation_tasks,
            core_space_address_network,
        ))
        .map_err(|error| startup_error(format!("failed to merge Conflux RPC module: {error}")))
}

fn create_provider_http_client() -> io::Result<HttpClient> {
    HttpClient::builder()
        .timeout(PROVIDER_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            startup_error(format!("failed to configure provider HTTP client: {error}"))
        })
}

fn create_ethereum_provider(
    config: &EthereumConfig,
    http_client: &HttpClient,
) -> io::Result<RootProvider> {
    let rpc_url = config
        .rpc_url
        .parse()
        .map_err(|error| configuration_error(format!("invalid Ethereum RPC URL: {error}")))?;

    Ok(RootProvider::new(RpcClient::new_http_with_client(
        http_client.clone(),
        rpc_url,
    )))
}

fn create_conflux_providers(
    config: &ConfluxConfig,
    http_client: &HttpClient,
) -> io::Result<(RootProvider, ConfluxProvider)> {
    let espace_rpc_url = config
        .espace_rpc_url
        .parse()
        .map_err(|error| configuration_error(format!("invalid eSpace RPC URL: {error}")))?;
    let core_space_rpc_url = config
        .core_space_rpc_url
        .parse()
        .map_err(|error| configuration_error(format!("invalid Core Space RPC URL: {error}")))?;
    let espace_provider = RootProvider::new(RpcClient::new_http_with_client(
        http_client.clone(),
        espace_rpc_url,
    ));
    let core_space_provider = ConfluxProvider::new(RpcClient::new_http_with_client(
        http_client.clone(),
        core_space_rpc_url,
    ));
    Ok((espace_provider, core_space_provider))
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn configuration_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

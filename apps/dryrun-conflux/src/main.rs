use std::{env, error::Error, io, net::SocketAddr, num::NonZeroUsize, sync::Arc, time::Duration};

use conflux_engine::{
    ConfluxEngine,
    config::{ConfluxChainConfig, ConfluxConfig, ConfluxRpcConfig},
};
use conflux_service::ConfluxService;
use jsonrpsee::server::Server;
use simulation_tasks::SimulationTaskSet;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod health;
mod rpc;

const DEFAULT_RPC_LISTEN_ADDR: &str = "127.0.0.1:8547";
const DEFAULT_HEALTH_LISTEN_ADDR: &str = "127.0.0.1:9001";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = conflux_config()?;
    let simulation_tasks = create_simulation_task_set()?;
    let native_address_network = config.chain.native_address_network;
    let engine = Arc::new(ConfluxEngine::new(config)?);
    let service = Arc::new(ConfluxService::new(engine, simulation_tasks.clone()));
    let module = rpc::build_rpc_module(service, native_address_network);

    let health_addr = health_listen_addr()?;
    let _health_handle = health::start_health_server(health_addr).await?;

    let rpc_addr = rpc_listen_addr()?;
    let server = Server::builder().build(rpc_addr).await?;
    let local_addr = server.local_addr()?;
    let handle = server.start(module);

    info!("dryrun-conflux RPC server started at {}", local_addr);

    handle.stopped().await;
    simulation_tasks.close();
    simulation_tasks.wait().await;

    Ok(())
}

fn conflux_config() -> Result<ConfluxConfig, Box<dyn Error>> {
    Ok(ConfluxConfig {
        chain: ConfluxChainConfig::mainnet(),
        rpc: ConfluxRpcConfig {
            evm_url: required_env("DRYRUN_CONFLUX_ESPACE_RPC_URL")?,
            native_url: required_env("DRYRUN_CONFLUX_NATIVE_RPC_URL")?,
        },
    })
}

fn create_simulation_task_set() -> Result<SimulationTaskSet, io::Error> {
    let max_concurrent = required_env("DRYRUN_CONFLUX_SIMULATION_MAX_CONCURRENT")?
        .parse::<usize>()
        .map_err(|error| {
            startup_error(format!(
                "DRYRUN_CONFLUX_SIMULATION_MAX_CONCURRENT must be an unsigned integer: {error}"
            ))
        })?;
    let max_concurrent = NonZeroUsize::new(max_concurrent).ok_or_else(|| {
        startup_error("DRYRUN_CONFLUX_SIMULATION_MAX_CONCURRENT must be greater than zero")
    })?;
    let admission_timeout_seconds = required_env("DRYRUN_CONFLUX_SIMULATION_ADMISSION_TIMEOUT_SECONDS")?
        .parse::<u64>()
        .map_err(|error| {
            startup_error(format!(
                "DRYRUN_CONFLUX_SIMULATION_ADMISSION_TIMEOUT_SECONDS must be an unsigned integer: {error}"
            ))
        })?;

    Ok(SimulationTaskSet::new(
        max_concurrent,
        Duration::from_secs(admission_timeout_seconds),
    ))
}

fn required_env(name: &'static str) -> Result<String, io::Error> {
    env::var(name).map_err(|_| startup_error(format!("{name} must be set")))
}

fn startup_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn rpc_listen_addr() -> Result<SocketAddr, Box<dyn Error>> {
    let value = env::var("DRYRUN_CONFLUX_LISTEN_ADDR")
        .unwrap_or_else(|_| DEFAULT_RPC_LISTEN_ADDR.to_owned());

    Ok(value.parse()?)
}

fn health_listen_addr() -> Result<SocketAddr, Box<dyn Error>> {
    let value = env::var("DRYRUN_CONFLUX_HEALTH_LISTEN_ADDR")
        .unwrap_or_else(|_| DEFAULT_HEALTH_LISTEN_ADDR.to_owned());

    Ok(value.parse()?)
}
